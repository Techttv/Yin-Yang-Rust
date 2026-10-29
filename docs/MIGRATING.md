# Passare a `yinyang` dal vecchio Yin-Yang

`yinyang` vive accanto al vecchio programma con nomi diversi (`yinyang`, `yinyang-gui`,
`yinyang.timer`, `~/.config/yinyang.json`). Il pacchetto non attiva nulla: il passaggio lo fai tu.

## 1. Installare

```sh
cd packaging/arch && makepkg -si
```

Se avevi installato una copia per utente in `~/.local` (con una versione precedente dello script
di installazione), togliila, perché in `PATH` verrebbe prima di `/usr/bin`:

```sh
rm -f ~/.local/bin/yinyang ~/.local/bin/yinyang-gui \
      ~/.local/share/applications/yinyang-gui.desktop \
      ~/.local/share/icons/hicolor/scalable/apps/yinyang.svg
hash -r
```

## 2. Provare senza cambiare nulla

```sh
yinyang plugins --json | jq 'map({key,available})'
yinyang apply --dark --force --dry-run     # stampa cosa farebbe, non esegue niente
yinyang status
```

`apply --auto --dry-run` dice "no location" finché una vera esecuzione non ha salvato una posizione:
un test a secco non usa la rete e non scrive nulla.

## 3. Passare

```sh
systemctl --user disable --now yin_yang.timer   # il vecchio timer
yinyang timer refresh                            # il nuovo: scrive e avvia yinyang.timer
systemctl --user list-timers yinyang.timer
```

Se avevi già un `yinyang.timer` creato da una copia in `~/.local`, `yinyang timer refresh` riscrive
il servizio con il percorso `/usr/bin/yinyang`.

## Tornare indietro

```sh
systemctl --user disable --now yinyang.timer
systemctl --user enable --now yin_yang.timer
```

## Avvertenze

- **Aprire la vecchia GUI riattiva `yin_yang.timer`**: il vecchio programma rigenera e avvia il
  proprio timer a ogni apertura o salvataggio. Disattivalo di nuovo con il primo comando del
  passo 3. Resta anche la voce di menu del vecchio programma, che può partire da un avvio automatico.
- Il primo `yinyang apply` reale o il tasto Apply della GUI abilitano `yinyang.timer`: fallo dopo
  aver disattivato quello vecchio, altrimenti due timer cambiano il tema.
- Firefox e Zen non hanno più un'integrazione: il loro tema predefinito segue il tema di sistema
  tramite il portale `org.freedesktop.appearance`.
- Disinstallare il pacchetto vecchio (`sudo pacman -Rns yin-yang`) è una tua scelta, quando sei
  sicuro; non è necessario per usare `yinyang`.
