#!/usr/bin/env python3
"""Decode le dialogue de controle du Kraken Elite dans une capture USBPcap.

Complement de `extrait_22.py`, qui ne lit que la charge utile et ignore le
sens du transfert. Ici le sens est l'information principale : la spec §3
liste les commandes envoyees par CAM, jamais les REPONSES du controleur.
C'est dans ces reponses que vit le protocole des « buckets », le stockage
d'images du Kraken, dont la spec §2 ne decrit pas la reservation.

Structure de l'en-tete USBPcap (USBPCAP_BUFFER_PACKET_HEADER) :

    0..1    longueur de l'en-tete       uint16
    2..9    identifiant d'IRP           uint64
    10..13  statut                      uint32
    14..15  fonction                    uint16
    16      info -- bit 0 : 1 = du peripherique vers l'hote
    17..18  bus                         uint16
    19..20  peripherique                uint16
    21      endpoint (bit 0x80 = IN)    uint8
    22      type de transfert           uint8
    23..26  longueur des donnees        uint32

Usage :
    extrait_kraken.py <fichier.pcap> <device>            dialogue complet
    extrait_kraken.py <fichier.pcap> <device> 32         seule la famille 0x32
"""

import struct
import sys
from datetime import datetime, timedelta, timezone

LOCAL = timezone(timedelta(hours=2))

# Type de transfert USBPcap, pour ne pas confondre les trames de controle de
# 64 octets avec les morceaux d'image, qui font 512 octets et n'ont pas de
# structure de commande.
INTERRUPT = 1
BULK = 2

FAMILLES = {
    0x10: "init",
    0x30: "affichage / luminosite",
    0x31: "reponse d etat d affichage",
    0x32: "buckets",
    0x33: "reponse buckets",
    0x36: "transfert d image",
    0x37: "reponse transfert",
    0x38: "mode d affichage",
    0x70: "?",
    0x72: "consignes pompe / ventilateur",
    0x74: "demande d etat",
    0x75: "reponse d etat",
}


def paquets(chemin):
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
            info = data[16]
            device = struct.unpack("<H", data[19:21])[0]
            endpoint = data[21]
            transfert = data[22]
            data_len = struct.unpack("<I", data[23:27])[0]
            charge = data[header_len:header_len + data_len]

            micro = ts_frac // 1000 if nano else ts_frac
            horo = datetime.fromtimestamp(ts_sec, LOCAL).replace(microsecond=micro)
            entrant = bool(info & 0x01)
            yield horo, device, endpoint, transfert, entrant, charge


def hexa(octets, taille=None):
    vus = octets if taille is None else octets[:taille]
    return " ".join(f"{o:02x}" for o in vus)


def utile(charge):
    """Derniere position non nulle, pour ne pas afficher 50 octets de bourrage."""
    fin = len(charge)
    while fin > 0 and charge[fin - 1] == 0:
        fin -= 1
    return fin


def main():
    if len(sys.argv) < 3:
        sys.exit("usage: extrait_kraken.py <fichier.pcap> <device> [famille hex]")

    chemin = sys.argv[1]
    cible = int(sys.argv[2])
    famille = int(sys.argv[3], 16) if len(sys.argv) > 3 else None

    total = 0
    volume_bulk = 0
    for horo, device, endpoint, transfert, entrant, charge in paquets(chemin):
        if device != cible or not charge:
            continue

        if transfert == BULK:
            volume_bulk += len(charge)
            continue
        if transfert != INTERRUPT:
            continue
        if famille is not None and charge[0] != famille:
            continue

        total += 1
        sens = "<--" if entrant else "-->"
        nom = FAMILLES.get(charge[0], "")
        fin = max(utile(charge), 2)
        marque = "" if fin <= 32 else f"  (+{len(charge) - 32} octets, dont {fin - 32} non nuls)"
        print(
            f"{horo:%H:%M:%S.%f}  ep{endpoint:02x} {sens}  "
            f"{hexa(charge, 32)}{marque}   {nom}"
        )

    print(f"\n{total} trames de controle", file=sys.stderr)
    if volume_bulk:
        print(f"{volume_bulk} octets en bulk (tronques par USBPcap)", file=sys.stderr)


if __name__ == "__main__":
    main()
