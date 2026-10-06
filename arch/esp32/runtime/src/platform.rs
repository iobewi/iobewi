//! S3 reference-profile startup. Existing flash, NVS, identity and Wi-Fi adapters
//! are reused; product mode/persistence/service policy remains outside startup.
use core::convert::Infallible;
use embassy_executor::Spawner;
use embassy_net::StackResources;
use esp_hal::{
    Blocking,
    peripherals::{GPIO19, GPIO20, Peripherals, RTC_TIMER, USB_DEVICE, USB_FS, WIFI},
    timer::timg::TimerGroup,
    uart::{Config as UartConfig, Uart},
    usb::usb_serial_jtag::UsbSerialJtag,
};
use iobewi_board::{
    Board, BoardParts, BootIo, BootIoFactory, Reset, ResourceRequest, Serial, UsbBootMode,
};
use iobewi_device::{DeviceIdentity, DeviceMetadata};
use iobewi_esp_config_space::{NvsConfigBackend, NvsStartupError};
use iobewi_esp_platform::profiles::{S3_NATIVE_USB, validate_resources};
use iobewi_esp_serial::{EspSerialBank, Rx, Tx};

/// Diagnostics only for fatal startup: one nonblocking FIFO attempt per byte,
/// no flush, retry, logger installation, host wait or USB/JTAG register access.
fn fatal(mut uart: Uart<'static, Blocking>, code: &[u8]) -> ! {
    for byte in code {
        if !uart.write_ready() {
            break;
        }
        let _ = uart.write(core::slice::from_ref(byte));
    }
    loop {
        core::hint::spin_loop();
    }
}

pub struct EspBootIo {
    uart: Uart<'static, Blocking>,
    jtag: USB_DEVICE<'static>,
    usb: USB_FS<'static>,
    dp: GPIO20<'static>,
    dm: GPIO19<'static>,
    out: &'static mut [u8],
}
impl BootIoFactory for EspBootIo {
    type Serial = EspSerialBank;
    type Driver = iobewi_esp_usb::Driver<'static>;
    type Error = Infallible;
    fn select(self, mode: UsbBootMode) -> Result<BootIo<Self::Serial, Self::Driver>, Infallible> {
        let (rx, tx) = self.uart.into_async().split();
        let uart = Serial {
            rx: Rx::Uart(rx),
            tx: Tx::Uart(tx),
        };
        let (jtag, usb) = match mode {
            UsbBootMode::Provisioning => {
                // OTG constructor is never called in this branch.
                let (rx, tx) = UsbSerialJtag::new(self.jtag).into_async().split();
                (
                    Some(Serial {
                        rx: Rx::Jtag(rx),
                        tx: Tx::Jtag(tx),
                    }),
                    None,
                )
            }
            UsbBootMode::MassStorage => {
                // USB_DEVICE is only dropped; Serial/JTAG is never initialized.
                (
                    None,
                    Some(iobewi_esp_usb::device(self.usb, self.dp, self.dm, self.out)),
                )
            }
        };
        Ok(BootIo {
            serial: EspSerialBank::new(uart, jtag),
            usb,
        })
    }
}

pub struct EspReset {
    rtc: RTC_TIMER<'static>,
}
impl Reset for EspReset {
    fn reset(self) -> ! {
        // RTC ResetSystem scope, not the digital-core-only software reset.
        let _rtc = iobewi_esp_reset::arm_system_reset(self.rtc, 100);
        loop {
            core::hint::spin_loop();
        }
    }
}
/// Delegates both existing identity providers; no product chip literal.
pub struct EspBoardIdentity;
impl DeviceIdentity for EspBoardIdentity {
    fn hardware_id(&self) -> alloc::string::String {
        iobewi_esp_device::EspDeviceIdentity.hardware_id()
    }
    fn mac_address(&self) -> Option<[u8; 6]> {
        iobewi_esp_device::EspDeviceIdentity.mac_address()
    }
}
impl DeviceMetadata for EspBoardIdentity {
    fn chip_name(&self) -> &'static str {
        iobewi_esp_device::EspDeviceMetadata.chip_name()
    }
    fn ram_size(&self) -> u32 {
        iobewi_esp_device::EspDeviceMetadata.ram_size()
    }
}
extern crate alloc;

pub struct EspBoard<const SOCKETS: usize> {
    parts: BoardParts<
        iobewi_esp_wifi::WifiManager<SOCKETS>,
        NvsConfigBackend,
        esp_hal::gpio::Input<'static>,
        EspBootIo,
        EspReset,
        EspBoardIdentity,
    >,
}
impl<const SOCKETS: usize> Board for EspBoard<SOCKETS> {
    type Wifi = iobewi_esp_wifi::WifiManager<SOCKETS>;
    type Config = NvsConfigBackend;
    type Button = esp_hal::gpio::Input<'static>;
    type Io = EspBootIo;
    type Reset = EspReset;
    type Identity = EspBoardIdentity;
    fn into_parts(
        self,
    ) -> BoardParts<Self::Wifi, Self::Config, Self::Button, Self::Io, Self::Reset, Self::Identity>
    {
        self.parts
    }
}

pub struct Startup<const SOCKETS: usize> {
    wifi: WIFI<'static>,
    resources: &'static mut StackResources<SOCKETS>,
    flash: &'static iobewi_esp_flash::SharedFlash,
    button: esp_hal::gpio::Input<'static>,
    io: EspBootIo,
    reset: EspReset,
}
pub struct StartupFailure {
    pub error: NvsStartupError,
    io: EspBootIo,
}
impl StartupFailure {
    pub fn halt(self) -> ! {
        fatal(self.io.uart, b"IOBEWI:NVS\r\n")
    }
}

impl<const SOCKETS: usize> Startup<SOCKETS> {
    /// Called once on the entry main stack; heap initializer is supplied by the
    /// expansion so its reservation uses the product's const request.
    pub fn prepare(
        p: Peripherals,
        request: ResourceRequest,
        resources: &'static mut StackResources<SOCKETS>,
        out: &'static mut [u8],
        initialize_heap: impl FnOnce(),
    ) -> Self {
        let uart = match Uart::new(p.UART0, UartConfig::default()) {
            Ok(uart) => uart.with_rx(p.GPIO44).with_tx(p.GPIO43),
            Err(_) => loop {
                core::hint::spin_loop();
            }, // UART itself unavailable.
        };
        if request.sockets != SOCKETS
            || SOCKETS == 0
            || validate_resources(
                request.heap_bytes,
                core::mem::size_of::<StackResources<SOCKETS>>(),
                out.len(),
                request.minimum_stack_bytes,
                S3_NATIVE_USB.resource_budget_bytes,
            )
            .is_err()
        {
            fatal(uart, b"IOBEWI:RAM\r\n");
        }
        unsafe extern "C" {
            static _stack_end: u8;
            static _stack_start: u8;
        }
        let linker_stack =
            (&raw const _stack_start as usize).saturating_sub(&raw const _stack_end as usize);
        if linker_stack < request.minimum_stack_bytes {
            fatal(uart, b"IOBEWI:STACK\r\n");
        }
        initialize_heap();
        let timer = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timer.timer0, p.FROM_CPU_INTR0);
        let flash = iobewi_esp_flash::init(p.FLASH);
        Self {
            wifi: p.WIFI,
            resources,
            flash,
            button: iobewi_esp_input::boot_button(p.GPIO0),
            io: EspBootIo {
                uart,
                jtag: p.USB_DEVICE,
                usb: p.USB_FS,
                dp: p.GPIO20,
                dm: p.GPIO19,
                out,
            },
            reset: EspReset { rtc: p.RTC_TIMER },
        }
    }
    /// Complete async NVS initialization before exposing an infallible Board split.
    /// No native USB controller has been initialized, even on the error path.
    pub async fn finish(self, spawner: Spawner) -> Result<EspBoard<SOCKETS>, StartupFailure> {
        let config = match NvsConfigBackend::from_label(self.flash, S3_NATIVE_USB.nvs_label).await {
            Ok(config) => config,
            Err(error) => return Err(StartupFailure { error, io: self.io }),
        };
        Ok(EspBoard {
            parts: BoardParts {
                wifi: iobewi_esp_wifi::WifiManager::new(self.wifi, spawner, self.resources),
                config,
                button: self.button,
                io: self.io,
                reset: self.reset,
                identity: EspBoardIdentity,
            },
        })
    }
}
impl iobewi_device::PinMetadata for EspBoardIdentity {
    fn pins(&self) -> &'static [iobewi_device::PinDescriptor] {
        iobewi_device::PinMetadata::pins(&iobewi_esp_device::EspDeviceMetadata)
    }
}
