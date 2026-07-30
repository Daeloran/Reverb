<#
    Reconstitue les transactions SMBus a partir des acces aux ports d'E/S
    captures par API Monitor.

    Le pilote Corsair n'expose que de l'E/S sur port brut. Le service pilote
    lui-meme le controleur SMBus AMD FCH, compatible PIIX4 :

        base+0  SMBHSTSTS   etat
        base+2  SMBHSTCNT   controle : bit6 = START, bits 4:2 = protocole
        base+3  SMBHSTCMD   registre vise
        base+4  SMBHSTADD   adresse 7 bits decalee, bit0 = lecture
        base+5  SMBHSTDAT0  donnee

    Sans accents (UTF-8 sans BOM + PowerShell 5.1).
#>
param(
    [Parameter(Mandatory = $true)][string] $Apmx,
    [int] $MaxTransactions = 250,
    [int] $Decalage = 40,
    [string] $Csv = '',
    [string] $CsvBrut = ''
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;

public class SmbusRebuild
{
    public class Access { public bool Write; public uint Port; public uint Value; }

    public static List<Access> Parse(Stream calls, Stream data, int decalage)
    {
        var idx = new List<long>();
        byte[] c8 = new byte[8];
        while (true)
        {
            int got = 0;
            while (got < 8) { int r = calls.Read(c8, got, 8 - got); if (r <= 0) break; got += r; }
            if (got < 8) break;
            idx.Add(BitConverter.ToInt64(c8, 0));
        }

        var list = new List<Access>();
        byte[] rec = new byte[65536];
        for (int k = 0; k + 1 < idx.Count; k++)
        {
            int len = (int)(idx[k + 1] - idx[k]);
            if (len <= 0 || len > rec.Length) break;
            int got = 0;
            while (got < len) { int r = data.Read(rec, got, len - got); if (r <= 0) break; got += r; }
            if (got < len) break;

            for (int i = 0; i + 4 <= len - decalage - 10; i++)
            {
                uint v = BitConverter.ToUInt32(rec, i);
                if ((v >> 16) == 0x22 && (v & 3) == 0 && v > 0x220000 && v < 0x230000)
                {
                    int b = i + decalage;
                    var a = new Access();
                    a.Write = (v == 0x229354);
                    a.Port  = BitConverter.ToUInt32(rec, b);
                    a.Value = BitConverter.ToUInt32(rec, b + 4);
                    list.Add(a);
                    break;
                }
            }
        }
        return list;
    }
}
'@

$zip = [System.IO.Compression.ZipFile]::OpenRead($Apmx)
try {
    $sC = ($zip.Entries | Where-Object { $_.FullName -like 'process/*/calls' } | Select-Object -First 1).Open()
    $sD = ($zip.Entries | Where-Object { $_.FullName -like 'process/*/data' }  | Select-Object -First 1).Open()
    try { $acc = [SmbusRebuild]::Parse($sC, $sD, $Decalage) } finally { $sC.Dispose(); $sD.Dispose() }
} finally { $zip.Dispose() }

"acces au bus analyses : $($acc.Count)"
$base = 0x0B00

if ($CsvBrut) {
    # Export des acces bruts, indispensable pour les transferts par bloc :
    # les donnees y transitent par le registre SMBBLKDAT, octet par octet,
    # et n'apparaissent donc pas dans la reconstitution par transaction.
    $sw = New-Object System.IO.StreamWriter($CsvBrut, $false, [System.Text.Encoding]::UTF8)
    $sw.WriteLine('Ordre,Sens,Port,Valeur')
    for ($i = 0; $i -lt $acc.Count; $i++) {
        $a = $acc[$i]
        if ($null -eq $a) { continue }
        $sens = 'R'
        if ($a.Write) { $sens = 'W' }
        # Concatenation plutot que -f : l'operateur de format se comportait mal
        # sur ces valeurs non signees.
        $p = '0x' + ([uint32]$a.Port).ToString('x4')
        $v = '0x' + ([uint32]$a.Value).ToString('x2')
        $sw.WriteLine([string]$i + ',' + $sens + ',' + $p + ',' + $v)
    }
    $sw.Close()
    "csv brut ecrit : $CsvBrut"
}

# reconstitution : on accumule adresse/registre/donnee, puis on emet la
# transaction au moment ou le registre de controle recoit un START.
$addr = $null; $cmd = $null; $dat = $null
$tx = @()
foreach ($a in $acc) {
    if (-not $a.Write) { continue }
    switch ($a.Port - $base) {
        4 { $addr = $a.Value }
        3 { $cmd  = $a.Value }
        5 { $dat  = $a.Value }
        2 {
            if ($a.Value -band 0x40) {   # bit START
                $proto = ($a.Value -shr 2) -band 0x7
                $tx += [pscustomobject]@{
                    Addr7    = if ($addr -ne $null) { [int]($addr -shr 1) } else { $null }
                    Lecture  = if ($addr -ne $null) { [bool]($addr -band 1) } else { $null }
                    Registre = $cmd
                    Donnee   = $dat
                    Proto    = $proto
                }
                if ($tx.Count -ge $MaxTransactions) { break }
            }
        }
    }
}

"transactions reconstituees : $($tx.Count)"

if ($Csv) {
    $tx | Select-Object @{n='Ordre';e={[array]::IndexOf($tx,$_)}},
        @{n='Addr';e={'0x{0:x2}' -f $_.Addr7}},
        @{n='Sens';e={ if ($_.Lecture) { 'R' } else { 'W' } }},
        @{n='Registre';e={'0x{0:x2}' -f $_.Registre}},
        @{n='Donnee';e={'0x{0:x2}' -f $_.Donnee}},
        Proto | Export-Csv -Path $Csv -NoTypeInformation -Encoding UTF8
    "csv ecrit : $Csv"
}
""
"===== adresses SMBus visees ====="
$tx | Where-Object { $_.Addr7 -ne $null } | Group-Object Addr7 | Sort-Object Name |
  ForEach-Object { "  0x{0:x2} : {1} transactions" -f [int]$_.Name, $_.Count }
""
"===== registres ecrits, par adresse ====="
$tx | Where-Object { $_.Addr7 -ne $null -and -not $_.Lecture } |
  Group-Object { '0x{0:x2}' -f $_.Addr7 }, { '0x{0:x2}' -f $_.Registre } |
  Sort-Object Count -Descending | Select-Object -First 30 |
  ForEach-Object { "  {0,6}x  addr/reg {1}" -f $_.Count, $_.Name }
""
"===== sequence detaillee (60 premieres) ====="
"{0,4}  {1,-6} {2,-8} {3,-8} {4,-8} {5}" -f '#','addr','sens','registre','donnee','proto'
$i = 0
foreach ($t in ($tx | Select-Object -First 60)) {
    $i++
    "{0,4}  0x{1:x2}   {2,-8} 0x{3:x2}     0x{4:x2}     {5}" -f `
        $i, $t.Addr7, $(if ($t.Lecture) { 'lecture' } else { 'ecriture' }), $t.Registre, $t.Donnee, $t.Proto
}
