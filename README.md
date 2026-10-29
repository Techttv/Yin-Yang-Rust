# yinyang

Cambia da solo il tema di KDE Plasma tra chiaro e scuro, a orari fissi oppure da alba a tramonto.
È una riscrittura nativa di [Yin-Yang](https://github.com/oskarsh/Yin-Yang) (MIT, Python/PySide6):
niente Python a runtime, quindi un aggiornamento di Arch o un `PATH` diverso non lo rompono.

- `yinyang`: programma a riga di comando in Rust (`apply`, `toggle`, `status`, `plugins`, `config`, `timer`).
  È lui a leggere e scrivere la configurazione e a gestire il timer systemd dell'utente.
- `yinyang-gui`: finestra e icona nel vassoio in Qt 6, che segue il tema di Plasma. Chiama solo il
  programma a riga di comando e non scrive mai la configurazione da sola.
- Solo KDE Plasma 6. Plugin: system, colors, gtk, icons, kvantum, wallpaper, vs code, onlyoffice,
  okular, konsole, custom, notification. Firefox e Zen seguono il tema di sistema da soli.

## Installazione (Arch Linux)

Il pacchetto si costruisce da questo repository con `makepkg`: pacman lo installa e lo conosce
(`pacman -Qi yinyang`, `sudo pacman -R yinyang`). Non serve AUR né un helper.

```sh
git clone <indirizzo-del-repository> yinyang
cd yinyang/packaging/arch
makepkg -si
```

Servono `rust`, `cmake`, `qt6-tools` e `git` per compilare: `-s` li installa. La compilazione esegue
anche i test (circa 4 minuti la prima volta).

Aggiornare:

```sh
git pull && cd packaging/arch && makepkg -si
```

Il pacchetto non attiva nessun timer. Per attivarlo (e per passare dal vecchio Yin-Yang) vedi
[docs/MIGRATING.md](docs/MIGRATING.md).

## Uso

```sh
yinyang plugins                       # plugin disponibili
yinyang apply --dark --force --dry-run # cosa farebbe, senza eseguire
yinyang-gui                           # imposta modalità, orari, plugin
yinyang timer refresh                 # (ri)crea e avvia il timer dell'utente
```

La configurazione è in `~/.config/yinyang.json`; se non c'è, viene importata (senza modificarla) quella
del vecchio Yin-Yang. Il progetto è descritto in [docs/design.md](docs/design.md).

## Licenza

MIT, come Yin-Yang di cui questo progetto riprende il comportamento, le traduzioni, l'icona e il
disegno della finestra: vedi [LICENSE](LICENSE).
