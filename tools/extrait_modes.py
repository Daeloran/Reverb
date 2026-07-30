#!/usr/bin/env python3
"""Extrait les trames 0x2a 0x04 d'une capture USBPcap et les rattache aux actions.

Format USBPcap (DLT 249) :
  en-tete pcap global : 24 octets
  par enregistrement  : ts_sec u32, ts_usec u32, incl_len u32, orig_len u32, puis donnees
  en-tete USBPcap     : headerLen u16 en offset 0, device u16 en 19,
                        endpoint u8 en 21, dataLength u32 en 23
  charge utile        : a partir de headerLen
"""
import struct
import sys
from datetime import datetime, timedelta, timezone

PCAP = sys.argv[1] if len(sys.argv) > 1 else None
LOCAL = timezone(timedelta(hours=2))  # horodatage du journal : +02:00

# (heure de fin d'action, libelle) — l'action s'applique AVANT cette heure
ACTIONS = [
    ("10:25:06", "2  fixe ROUGE, tous les canaux"),
    ("10:25:21", "3  fixe BLEU, un seul ventilateur"),
    ("10:25:50", "4  fixe BLEU, un autre ventilateur"),
    ("10:26:48", "5  luminosite MAXIMUM"),
    ("10:27:19", "6  luminosite MINIMUM"),
    ("10:28:46", "7  mode BREATHING rouge"),
    ("10:29:06", "8  mode SPECTRUM WAVE"),
    ("10:29:51", "9  changement de VITESSE"),
    ("10:30:37", "10 changement de DIRECTION"),
    ("10:32:23", "11 mode MULTICOLORE, 3 couleurs"),
    ("10:34:53", "12 LED individuelles"),
    ("10:37:21", "13 fixe MAGENTA, tous les canaux"),
]


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
            endpoint = data[21]
            data_len = struct.unpack("<I", data[23:27])[0]
            charge = data[header_len:header_len + data_len]

            micro = ts_frac // 1000 if nano else ts_frac
            horo = datetime.fromtimestamp(ts_sec, LOCAL).replace(microsecond=micro)
            yield horo, device, endpoint, charge


def action_de(horo):
    hhmmss = horo.strftime("%H:%M:%S")
    for fin, libelle in ACTIONS:
        if hhmmss <= fin:
            return libelle
    return "apres la derniere action"


def main():
    if not PCAP:
        sys.exit("usage: extrait_modes.py <fichier.pcap>")

    par_action = {}
    total = 0

    for horo, device, endpoint, charge in trames(PCAP):
        if len(charge) < 2 or charge[0] != 0x2A or charge[1] != 0x04:
            continue
        total += 1
        cle = action_de(horo)
        # dedoublonne : CAM reemet la meme trame en boucle
        par_action.setdefault(cle, {})
        signature = bytes(charge[:64])
        par_action[cle].setdefault(signature, [0, horo, device])
        par_action[cle][signature][0] += 1

    print(f"{total} trames 0x2a 0x04 au total\n")

    for libelle, _ in [(l, None) for _, l in ACTIONS] + [("apres la derniere action", None)]:
        if libelle not in par_action:
            continue
        uniques = par_action[libelle]
        print(f"{'=' * 78}")
        print(f"ACTION {libelle}   — {len(uniques)} trame(s) distincte(s)")
        print(f"{'=' * 78}")
        for sig, (compte, horo, device) in sorted(
            uniques.items(), key=lambda kv: -kv[1][0]
        ):
            o = list(sig)
            couleurs = []
            nb = o[56] if len(o) > 56 else 0
            for i in range(max(nb, 1)):
                base = 7 + i * 3
                if base + 2 < len(o):
                    g, r, b = o[base], o[base + 1], o[base + 2]
                    couleurs.append(f"#{r:02x}{g:02x}{b:02x}")
            print(
                f"  dev{device} ×{compte:<4} "
                f"masque={o[2]:#04x}/{o[3]:#04x} mode={o[4]:#04x} "
                f"vitesse={o[5]:#04x} off6={o[6]:#04x} "
                f"| off56={o[56]:#04x} off57={o[57]:#04x} "
                f"off58={o[58]:#04x} off59={o[59]:#04x}"
            )
            print(f"        couleurs RGB : {', '.join(couleurs)}")
        print()


if __name__ == "__main__":
    main()
