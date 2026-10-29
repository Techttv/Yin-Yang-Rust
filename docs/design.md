# Yin-Yang nativo (KDE) — specifica

Riscrittura di [oskarsh/Yin-Yang](https://github.com/oskarsh/Yin-Yang) 4.0.1 (MIT, Python/PySide6) senza dipendenza da Python.
Riferimento funzionale: i sorgenti in `/opt/yin_yang/yin_yang` (copiati nel repo in `reference/`).

## Perché
Il pacchetto Arch lancia `python3` dal `PATH` (qui: miniconda) e dipende da librerie Python di sistema (`suntime`, `dateutil`, `PySide6`, `systemd-python`). Basta un aggiornamento o un `PATH` diverso e il timer systemd fallisce. L'obiettivo è un programma che non cambia comportamento con gli aggiornamenti di Arch.

## Decisioni
- **Nucleo in Rust** (`yinyang`): CLI, calcolo alba/tramonto, timer systemd, tutti i plugin. Nessun toolkit grafico, nessun Python. Dipende solo da glibc.
- **GUI in C++/Qt6 Widgets** (`yinyang-gui`): solo API pubbliche di Qt (compatibili a livello binario in tutta la serie 6.x). Prende da sola tema, colori, icone e font di KDE tramite `plasma-integration`. Nessun KDE Frameworks.
- **La GUI non scrive mai la configurazione**: parla con il nucleo via CLI (`yinyang config show`, `config save`, `plugins --json`, `apply`, `toggle`). Lo schema è definito in un solo posto.
- **Solo KDE Plasma 6**. Su altri desktop i plugin dipendenti dal desktop risultano "non disponibili", senza errori.
- **12 plugin**: system, colors, gtk, icons, kvantum, wallpaper, vs code, onlyoffice, okular, konsole, custom, notification.
- **Nessuna integrazione Firefox/Zen**: il loro tema predefinito segue il tema di sistema (portale `org.freedesktop.appearance`), quindi cambia insieme a Plasma. Niente estensione, host di native messaging né manifest. Si potrà aggiungere in seguito se servirà un tema specifico per il browser.
- **Convivenza con il vecchio programma**: nomi nuovi (`yinyang`, `yinyang.timer`, `~/.config/yinyang.json`). La configurazione vecchia viene importata, mai modificata.
- **Installazione per utente** in `~/.local` (nessun `sudo`, nessun pacchetto pacman nel ciclo di vita).

## Correzioni rispetto all'originale
- #84: `settings.json` di VS Code (JSONC con commenti) non viene più sovrascritto: si cambia solo il valore del tema.
- #360 / posizione: senza rete non si va in crash e non si usa mai (0,0); si usa l'ultima posizione nota.
- #199: wallpaper con `#` nel nome del file.
- #344: `--systemd` non esiste più; il service lancia `yinyang apply --auto`.
- Il timer ha `Persistent=true`: se il PC era spento all'ora del cambio, il cambio avviene all'accensione (un timer `OnCalendar` scaduto durante la sospensione scatta comunque al risveglio). Il service parte dopo `graphical-session.target` e `dark_mode` non si salva se un plugin fallisce, così il run successivo ritenta.
- In modalità "sunset to sunrise" il timer si riallinea a ogni esecuzione, anche quando il tema non cambia (l'alba si sposta ogni giorno).
- `--dry-run` non esegue davvero nulla.

## Fuori ambito (per ora)
GNOME, XFCE, MATE, Cinnamon, Budgie; eseguire yinyang stesso in un Flatpak (restano però i percorsi di configurazione e delle estensioni delle app Flatpak di VS Code e OnlyOffice); migrazione di configurazioni precedenti alla 4.0 (si usano i default); GeoClue2 (si usa la posizione IP); pacchetto PKGBUILD.
