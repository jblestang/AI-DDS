#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WIRE="$ROOT/interop/wire"
mkdir -p "$WIRE"

ROOT="$ROOT" WIRE="$WIRE" python3 << 'PY'
import os, socket, subprocess

root = os.environ["ROOT"]
wire = os.environ["WIRE"]
domain = int(os.environ.get("AIDDS_INTEROP_DOMAIN", "70"))
port = 7400 + 250 * domain
mcast = "239.255.0.1"
bin_dir = os.environ.get("AIDDS_INTEROP_BIN", f"{root}/target/interop-cyclonedds")

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
sock.bind(("", port))
mreq = socket.inet_aton(mcast) + socket.inet_aton("0.0.0.0")
sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
sock.settimeout(1)

env = {**os.environ, "AIDDS_INTEROP_DOMAIN": str(domain), "AIDDS_INTEROP_WAIT_MATCH": "0"}
proc = subprocess.Popen([f"{bin_dir}/interop_publisher", "1", "fixture"], env=env)
packets = []
for _ in range(5):
    try:
        packets.append(sock.recvfrom(65535)[0])
    except socket.timeout:
        pass
proc.wait(timeout=5)

wire = os.environ["WIRE"]
if packets:
    open(f"{wire}/cyclonedds_spdp.bin", "wb").write(packets[0])
    open(f"{wire}/cyclonedds_data_cdr_le.bin", "wb").write(max(packets, key=len))
    if len(packets) > 1:
        open(f"{wire}/cyclonedds_sedp.bin", "wb").write(packets[1])
    print(f"Captured {len(packets)} packets to {wire}/")
else:
    raise SystemExit("No packets captured; is CycloneDDS built?")
PY
