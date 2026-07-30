#!/usr/bin/env python3
"""TRACE D'UNE HYPOTHESE FAUSSE — conserve volontairement.

Tentative de piloter les 6 canaux RGB du NZXT RGB & Fan Controller (0x2019)
avec le protocole HUE 2 classique de liquidctl.

liquidctl declare `color_channel_count: 0` pour ce PID ("protocol changed,
see #541"). L'hypothese testee ici etait qu'il s'agissait d'une precaution
excessive, OpenRGB detectant le peripherique comme un Hue 2 standard a
6 canaux.

RESULTAT : ECHEC. Les trames partent sans erreur, aucune LED ne s'allume.
Le protocole de cette generation a reellement change.

Le vrai protocole a ete obtenu par retro-ingenierie le 2026-07-30 et vit
dans le vault : `02 - Projets/Reverb/SPEC-PROTOCOLE-NZXT.md`. Les commandes
sont `0x2a 0x04` (modes) et `0x22 0x10/0x11/0xa0` (LED par LED), avec les
couleurs en GRB.

Conserve pour eviter que quelqu'un — moi compris — retente cette voie.

Usage: uv run --with liquidctl tools/set_color_2019.py RRGGBB
"""
import sys

import liquidctl.driver.usb as _usb
_usb.PyUsbDevice.enumerate = staticmethod(lambda *a, **k: iter(()))

from liquidctl.driver.smart_device import SmartDevice2

# Reecrit l'entree 0x2019: 6 canaux couleur au lieu de 0
SmartDevice2._MATCHES = [
    (vid, pid, desc, {**opts, 'color_channel_count': 6} if pid == 0x2019 else opts)
    for (vid, pid, desc, opts) in SmartDevice2._MATCHES
]

rgb = sys.argv[1] if len(sys.argv) > 1 else "ff00ff"
color = [int(rgb[0:2], 16), int(rgb[2:4], 16), int(rgb[4:6], 16)]
print(f"Couleur demandee: #{rgb} -> RGB{tuple(color)}")

devs = [d for d in SmartDevice2.find_supported_devices() if d.product_id == 0x2019]
if not devs:
    sys.exit("Aucun controleur 0x2019 trouve")

for d in devs:
    print(f"\n--- {d.description} ({d.address})")
    print(f"    canaux couleur exposes: {list(d._color_channels)}")
    d.connect()
    try:
        for ch in ("led1", "led2", "led3", "led4", "led5", "led6"):
            d.set_color(ch, "fixed", [color])
            print(f"    {ch}: trame envoyee")
    except Exception as e:
        print(f"    ECHEC: {type(e).__name__}: {e}")
    finally:
        d.disconnect()

print("\nTermine. Rappel : aucune LED ne changera.")
