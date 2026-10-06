#!/usr/bin/env python3
"""Verify ONE established STA TCP connection; never reconnect silently."""
import argparse
import socket
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("station_ip")
parser.add_argument("--seconds", type=int, default=180)
args = parser.parse_args()
if args.seconds <= 0:
    parser.error("--seconds must be positive")
with socket.create_connection((args.station_ip, 3131), timeout=3) as connection:
    deadline = time.monotonic() + args.seconds
    count = 0
    while time.monotonic() < deadline:
        payload = count.to_bytes(8, "big")
        connection.sendall(payload)
        reply = bytearray()
        while len(reply) < len(payload):
            chunk = connection.recv(len(payload) - len(reply))
            if not chunk:
                raise RuntimeError("FAIL: established TCP connection closed")
            reply.extend(chunk)
        if reply != payload:
            raise RuntimeError("FAIL: corrupted echo")
        count += 1
        time.sleep(0.05)
print(f"PASS: {count} exchanges on one connection (no reconnect)")
