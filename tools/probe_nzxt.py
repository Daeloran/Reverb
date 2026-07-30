#!/usr/bin/env python3
"""Sonde read-only des controleurs NZXT.

N'envoie que des requetes d'information (0x10 0x01 = firmware,
0x20 0x03 = lighting info). Aucune ecriture de configuration.

Usage: uv run --with liquidctl tools/probe_nzxt.py
"""
import sys

# Neutralise l'enumeration pyusb: le Kraken Elite n'a pas de regle udev,
# et liquidctl plante dessus. On n'a besoin que de l'interface HID.
import liquidctl.driver.usb as _usb
_usb.PyUsbDevice.enumerate = staticmethod(lambda *a, **k: iter(()))
import liquidctl.driver.kraken3 as _k3
_k3.PyUsbDevice.enumerate = staticmethod(lambda *a, **k: iter(()))

from liquidctl.driver.smart_device import SmartDevice2, Nzxt2023RgbController
from liquidctl.driver.kraken3 import KrakenZ3
from liquidctl.util import Hue2Accessory, HUE2_MAX_ACCESSORIES_IN_CHANNEL


def read_until(dev, prefixes, max_attempts=12):
    """Lit des rapports jusqu'a avoir vu tous les prefixes demandes."""
    found = {}
    remaining = set(prefixes)
    for _ in range(max_attempts):
        if not remaining:
            break
        msg = dev.device.read(64)
        for p in list(remaining):
            if msg[0:len(p)] == list(p):
                found[p] = msg
                remaining.discard(p)
    return found


def parse_lighting(msg):
    """Parse la reponse 0x21 0x03: nb de canaux + accessoires par canal."""
    channel_count = msg[14]
    out = []
    offset = 15
    for c in range(channel_count):
        accs = []
        for a in range(HUE2_MAX_ACCESSORIES_IN_CHANNEL):
            aid = msg[offset + c * HUE2_MAX_ACCESSORIES_IN_CHANNEL + a]
            if aid == 0:
                break
            accs.append((aid, Hue2Accessory(aid)))
        out.append(accs)
    return out


def probe(dev, label):
    print(f"\n{'=' * 70}")
    print(f"  {label}")
    print(f"  {dev.description}")
    print(f"  VID:PID = {dev.vendor_id:04x}:{dev.product_id:04x}  "
          f"chemin HID = {dev.address}")
    print(f"{'=' * 70}")

    dev.connect()
    try:
        dev.device.clear_enqueued_reports()

        dev._write([0x10, 0x01])   # firmware info
        dev._write([0x20, 0x03])   # lighting info
        got = read_until(dev, [b'\x11\x01', b'\x21\x03'])

        fw = got.get(b'\x11\x01')
        if fw:
            print(f"  Firmware      : {fw[0x11]}.{fw[0x12]}.{fw[0x13]}")
        else:
            print("  Firmware      : pas de reponse")

        li = got.get(b'\x21\x03')
        if not li:
            print("  Canaux RGB    : pas de reponse a 0x20 0x03")
        else:
            chans = parse_lighting(li)
            print(f"  Canaux RGB    : {len(chans)}")
            total = 0
            for i, accs in enumerate(chans, 1):
                if not accs:
                    print(f"    - canal {i} : vide")
                    continue
                total += len(accs)
                print(f"    - canal {i} : {len(accs)} accessoire(s)")
                for j, (aid, acc) in enumerate(accs, 1):
                    print(f"        [{j}] id=0x{aid:02x}  {acc}")
            print(f"  Total accessoires RGB : {total}")

        # Etat courant (ventilos / temperatures), sans ecriture
        try:
            status = dev.get_status(direct_access=False)
            if status:
                print("  Etat :")
                for k, v, u in status:
                    print(f"    {k:<28} {v} {u}")
        except Exception as e:
            print(f"  Etat : indisponible ({type(e).__name__}: {e})")
    finally:
        dev.disconnect()


def main():
    found_any = False
    for cls, label in [
        (SmartDevice2, "NZXT RGB & Fan Controller"),
        (Nzxt2023RgbController, "NZXT 2023 RGB Controller"),
        (KrakenZ3, "NZXT Kraken"),
    ]:
        try:
            devs = list(cls.find_supported_devices())
        except Exception as e:
            print(f"[{label}] enumeration impossible: {type(e).__name__}: {e}")
            continue
        for d in devs:
            found_any = True
            try:
                probe(d, label)
            except Exception as e:
                print(f"  ERREUR sur {label}: {type(e).__name__}: {e}")

    if not found_any:
        print("Aucun peripherique NZXT trouve.", file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
