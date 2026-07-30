#!/usr/bin/env python3
"""Extrait les trames de la famille 0x22 d'une capture USBPcap.

Complement de `extrait_modes.py`, qui ne regarde que `0x2a 0x04`. Meme
format de fichier, meme lecture d'en-tete — voir la docstring de ce
script-la pour la structure USBPcap.

Sert a repondre a une question precise : la trame d'application `22 a0`
n'est reproduite qu'en statique dans la spec §5.2. Que valent ses octets
en mode anime ?
"""

import struct
import sys
from datetime import datetime, timedelta, timezone

LOCAL = timezone(timedelta(hours=2))


def trames(chemin):
    with open(chemin, "rb") as f:
        entete = f.read(24)
        if len(entete) < 24:
            sys.exit("fichier trop court")
        magic = struct.unpack("<I", entete[:4])[0]
        if magic not in (0xA1B2C3D4, 0xA1B23C4D):
            sys.exit(f"magic pcap inattendu : {magic:#x}")
        nano = magic == 0xA1B23C4D

        while True:
            rec = f.read(16)
            if len(rec) < 16:
                return
            ts_sec, ts_frac, incl, _orig = struct.unpack("<IIII", rec)
            data = f.read(incl)
            if len(data) < incl or incl < 27:
                return

            header_len = struct.unpack("<H", data[0:2])[0]
            device = struct.unpack("<H", data[19:21])[0]
            data_len = struct.unpack("<I", data[23:27])[0]
            charge = data[header_len:header_len + data_len]

            micro = ts_frac // 1000 if nano else ts_frac
            horo = datetime.fromtimestamp(ts_sec, LOCAL).replace(microsecond=micro)
            yield horo, device, charge


def hexa(octets):
    return " ".join(f"{o:02x}" for o in octets)


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: extrait_22.py <fichier.pcap> [sous-commande]")

    chemin = sys.argv[1]
    filtre = int(sys.argv[2], 16) if len(sys.argv) > 2 else None

    vues = {}
    ordre = []

    for horo, device, charge in trames(chemin):
        if len(charge) < 2 or charge[0] != 0x22:
            continue
        if filtre is not None and charge[1] != filtre:
            continue

        signature = bytes(charge[:64])
        if signature not in vues:
            vues[signature] = [0, horo, device]
            ordre.append(signature)
        vues[signature][0] += 1

    print(f"{len(ordre)} trame(s) 0x22 distincte(s)\n")

    for signature in ordre:
        compte, horo, device = vues[signature]
        o = list(signature)
        print(f"{horo:%H:%M:%S.%f}  dev{device}  ×{compte}  sous-commande {o[1]:#04x}")
        print(f"    0..15  {hexa(o[0:16])}")
        print(f"   16..31  {hexa(o[16:32])}")
        if any(o[32:]):
            print(f"   32..47  {hexa(o[32:48])}")
            print(f"   48..63  {hexa(o[48:64])}")
        print()


if __name__ == "__main__":
    main()
