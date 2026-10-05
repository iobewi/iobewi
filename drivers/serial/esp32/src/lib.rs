#![no_std]
use core::convert::Infallible as JtagError;
use embedded_io_async::{Error, ErrorKind, ErrorType, Read, Write};
use esp_hal::{
    Async,
    uart::{RxError, TxError, UartRx, UartTx},
    usb::usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};
use iobewi_board::{Serial, SerialBank};

pub enum Rx {
    Uart(UartRx<'static, Async>),
    Jtag(UsbSerialJtagRx<'static, Async>),
}
pub enum Tx {
    Uart(UartTx<'static, Async>),
    Jtag(UsbSerialJtagTx<'static, Async>),
}
#[derive(Debug)]
pub enum SerialError {
    UartRx(RxError),
    UartTx(TxError),
    Jtag(JtagError),
}
impl core::fmt::Display for SerialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl core::error::Error for SerialError {}
impl Error for SerialError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::UartRx(e) => e.kind(),
            Self::UartTx(e) => e.kind(),
            Self::Jtag(e) => e.kind(),
        }
    }
}
impl ErrorType for Rx {
    type Error = SerialError;
}
impl ErrorType for Tx {
    type Error = SerialError;
}
impl Read for Rx {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        match self {
            Self::Uart(rx) => Read::read(rx, buf).await.map_err(SerialError::UartRx),
            Self::Jtag(rx) => Read::read(rx, buf).await.map_err(SerialError::Jtag),
        }
    }
}
impl Write for Tx {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, SerialError> {
        match self {
            Self::Uart(tx) => Write::write(tx, buf).await.map_err(SerialError::UartTx),
            Self::Jtag(tx) => Write::write(tx, buf).await.map_err(SerialError::Jtag),
        }
    }
    async fn flush(&mut self) -> Result<(), SerialError> {
        match self {
            Self::Uart(tx) => Write::flush(tx).await.map_err(SerialError::UartTx),
            Self::Jtag(tx) => Write::flush(tx).await.map_err(SerialError::Jtag),
        }
    }
}
/// Fixed-capacity bank; each occupied slot transfers once, with no replenishment.
pub struct EspSerialBank {
    ports: [Option<Serial<Rx, Tx>>; 2],
    cursor: usize,
}
impl EspSerialBank {
    pub fn new(uart: Serial<Rx, Tx>, jtag: Option<Serial<Rx, Tx>>) -> Self {
        Self {
            ports: [Some(uart), jtag],
            cursor: 0,
        }
    }
}
impl SerialBank for EspSerialBank {
    type Rx = Rx;
    type Tx = Tx;
    fn take_next(&mut self) -> Option<Serial<Rx, Tx>> {
        while self.cursor < self.ports.len() {
            let index = self.cursor;
            self.cursor += 1;
            if let Some(port) = self.ports[index].take() {
                return Some(port);
            }
        }
        None
    }
}
