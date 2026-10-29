# Passaggio dal vecchio Yin-Yang a `yinyang`

Il nuovo programma vive accanto al vecchio con nomi diversi: `yinyang`, `yinyang-gui`,
`yinyang.timer`, `~/.config/yinyang.json`. Il vecchio `yin_yang` (pacchetto pacman `yin-yang`)
non viene toccato. Quello che segue riguarda il momento in cui **tu** decidi di passare.

## Installare (non cambia nulla del sistema oltre a `~/.local`)

```sh
./install.sh
```

Installa `yinyang` e `yinyang-gui` in `~/.local/bin`, il file `.desktop` in
`~/.local/share/applications` e l'icona nel tema `hicolor`. Non attiva nessun timer.
La prima volta `yinyang` importa `~/.config/yin_yang*.json` (la vecchia configurazione non viene mai
modificata) e salva la sua in `~/.config/yinyang.json`.

## Provare senza cambiare nulla

```sh
yinyang plugins --json | jq 'map({key,available})'
yinyang apply --dark --force --dry-run     # stampa cosa farebbe, non esegue niente
yinyang status
```

`apply --auto --dry-run` dice "no location" finché una vera esecuzione non ha salvato una posizione:
un test a secco non usa la rete e non scrive nulla.

## Passare

```sh
systemctl --user disable --now yin_yang.timer
yinyang timer refresh
systemctl --user list-timers yinyang.timer
```

## Tornare indietro

```sh
systemctl --user disable --now yinyang.timer
systemctl --user enable --now yin_yang.timer
```

## Avvertenze

- **Aprire la vecchia GUI riattiva `yin_yang.timer`.** Il vecchio programma rigenera e avvia il
  proprio timer ogni volta che si apre o si salva la configurazione (`daemon_handler.py`). Se lo
  fai, avrai due timer che cambiano il tema: disattiva di nuovo quello vecchio con il primo comando
  della sezione "Passare". Resta anche la voce di menu `~/.local/share/applications/yin_yang.desktop`,
  che avvia il vecchio programma (`yin_yang --systemd --minimized`) e può essere lanciata anche da un
  avvio automatico della sessione.
- Il vecchio timer oggi fallisce da solo (`ModuleNotFoundError: No module named 'suntime'`, perché
  `python3` nel `PATH` è quello di miniconda): è il motivo di questo programma.
- Firefox e Zen non hanno più un'integrazione: il loro tema predefinito segue il tema di sistema
  tramite il portale `org.freedesktop.appearance`, quindi cambia insieme a Plasma.
- Disinstallare il pacchetto `yin-yang` (`sudo pacman -Rns yin-yang`) è una tua scelta, da fare
  quando sei sicuro; non è necessario per usare `yinyang`.
