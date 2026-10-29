# Yin-Yang nativo (KDE) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sostituire Yin-Yang 4.0.1 (Python) con un nucleo Rust + GUI Qt6 nativa che funziona su KDE Plasma 6 senza dipendere da Python.

**Architecture:** Un crate Rust `yinyang` (libreria + binario) contiene configurazione, pianificazione, plugin e timer systemd, ed espone una CLI con output JSON. Una GUI C++/Qt6 Widgets (`gui/`) è un puro client di quella CLI via `QProcess`. Le chiamate a processi e a D-Bus passano da due trait (`Exec`, `Bus`) sostituibili nei test.

**Tech Stack:** Rust 1.99 (edition 2024), `clap`, `serde`/`serde_json`, `chrono`, `sunrise`, `ureq` (rustls), `zbus` (blocking), `anyhow`, `log`/`env_logger`, `tempfile`; C++17, Qt 6 Widgets + QtTest, CMake.

**Spec:** `docs/superpowers/specs/2026-10-29-yinyang-native-design.md`

## Global Constraints

- Solo KDE Plasma 6; su altri desktop i plugin dipendenti dal desktop (`system`, `colors`, `gtk`, `icons`, `wallpaper`) sono `available == false`, mai un errore.
- Dipendenze a runtime: glibc, Qt6 base (Core, Gui, Widgets). Vietati: Python, OpenSSL, KDE Frameworks, `systemd-python`/libsystemd.
- Binari: `yinyang` e `yinyang-gui`, installati in `~/.local/bin`. Nessun `sudo`.
- Config: `~/.config/yinyang.json`, schema `version: 4.0`, stesse chiavi dell'originale (incluse `"vs code"` con lo spazio e `wallpaper.light_theme: null`). Modi: `"manual"`, `"manual time"`, `"sunset to sunrise"`. Orari `"HH:MM"` in scrittura; in lettura si accettano `"HH:MM"` e `"HH:MM:SS"` (l'originale scrive il secondo).
- Config con `version` < 4.0: non si migrano; si usano i default e si emette un avviso.
- Unit systemd: `~/.local/share/systemd/user/yinyang.service` (`Type=oneshot`, `After=graphical-session.target`, `ExecStart=<exe> apply --auto`) e `yinyang.timer` (`Persistent=true`, due `OnCalendar=HH:MM:SS`, `OnStartupSec=<boot_offset>`).
- File vecchi (`~/.config/yin_yang_dev.json`, `~/.config/yin_yang.json`, unit `yin_yang.*`) si leggono o si ignorano, mai si modificano.
- Ogni sottocomando che scrive (`apply`, `toggle`, `config save`, `timer refresh`) prende il lock esclusivo `state_dir/lock` e solo dopo carica la config, modifica e salva.
- Un plugin che fallisce non impedisce gli altri. `dark_mode` si salva solo se nessun plugin abilitato ha restituito errore (così il run successivo ritenta). Un plugin abilitato ma non disponibile è "saltato" con avviso, non errore. L'exit code di `apply` è 1 se almeno un plugin ha fallito.
- `--dry-run` non esegue nulla: né `apply()` dei plugin, né scritture, né `systemctl`. Stampa soltanto cosa farebbe.
- Mai riscrivere un file utente (VS Code, Okular, OnlyOffice, Konsole) se non si è riusciti a leggerlo; si cambia solo il valore interessato, il resto resta byte per byte; si conservano permessi e si scrive sul percorso risolto dei link simbolici.
- La GUI non scrive mai il file di configurazione; usa solo la CLI.
- Qt: solo API pubbliche, `find_package(Qt6 6.5 REQUIRED COMPONENTS Widgets Test LinguistTools)`.
- Test del nucleo: nessun accesso a rete, D-Bus o `$HOME` reale (si usano `Paths::under(tmp)`, `FakeExec`, `FakeBus`).

## Review Focus

1. `settings.json` di VS Code con commenti `//`, `/* */` e virgole finali → resta identico tranne il valore del tema (Task 6, 7).
2. `update_location: true`, nessuna rete e nessuna posizione salvata → non si cambia tema e non si usa (0,0); errore chiaro (Task 2, 11).
3. Latitudini polari senza alba o senza tramonto → si usano gli orari manuali di `times` con un avviso (Task 2).
4. Pianificazione che attraversa la mezzanotte, confini esatti e `light == dark` (Task 2).
5. Programma o servizio D-Bus mancante, nessuna sessione D-Bus, Konsole non in esecuzione → plugin non disponibile o errore isolato; `plugins --json` e `config show` funzionano comunque (Task 3, 4, 5, 8, 11).
6. Il timer di `sunset to sunrise` si riallinea a ogni esecuzione, anche quando il tema non cambia (Task 10, 11).
7. Salvataggio dalla GUI con la finestra aperta mentre il timer ha cambiato tema → `dark_mode` non torna indietro (Task 11).

---

### Task 1: Scheletro, riferimento Python e configurazione

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/lib.rs`, `src/main.rs`, `src/paths.rs`, `src/config.rs`, `tests/config.rs`, `tests/fixtures/yin_yang_dev.json`
- Create: `reference/` (copia di `/opt/yin_yang/yin_yang`, `/opt/yin_yang/designer`, `/opt/yin_yang/resources`, più `LICENSE` copiato da `/usr/share/licenses/yin-yang/MIT`)

**Interfaces:**
- Produces:
  - `Paths { home: PathBuf, config_file: PathBuf, state_dir: PathBuf, systemd_dir: PathBuf }`; `Paths::from_env() -> Paths`; `Paths::under(home: &Path) -> Paths` (`config_file = home/.config/yinyang.json`, `state_dir = home/.local/state/yinyang`, `systemd_dir = home/.local/share/systemd/user`).
  - `enum Mode { Manual, Scheduled, FollowSun }` con serde rename ai tre stringhe del vincolo globale.
  - `struct PluginConfig { enabled: bool, light_theme: Option<String>, dark_theme: Option<String> }`.
  - `struct Config { version: f64, running: bool, dark_mode: bool, mode: Mode, coordinates: (f64, f64), update_location: bool, update_interval: u32, times: (NaiveTime, NaiveTime), boot_offset: u32, plugins: BTreeMap<String, PluginConfig> }`.
  - `Config::load(paths: &Paths) -> anyhow::Result<Config>`: se `config_file` non esiste importa il più recente tra `~/.config/yin_yang_dev.json` e `~/.config/yin_yang.json`; se non ce ne sono, o hanno `version` < 4.0, restituisce i default (con `log::warn!` nel secondo caso). Non modifica mai i file vecchi.
  - `Config::save(&self, paths: &Paths) -> anyhow::Result<()>`: scrittura atomica (file temporaneo nella stessa cartella + `rename`).
  - `Config::fill_defaults(&mut self, defaults: &[(String, Option<String>, Option<String>)])`: aggiunge (disabilitati) i plugin mancanti.

- [ ] **Step 1: `git init`, creare `Cargo.toml` (crate `yinyang`, lib + bin) e copiare `reference/` con `LICENSE`**

Run: `cargo build`
Expected: compila con `main.rs` vuoto.

- [ ] **Step 2: Copiare la config reale in `tests/fixtures/yin_yang_dev.json` e scrivere i test in `tests/config.rs`**

```rust
#[test] fn loads_real_config_fixture() {
    // home temporaneo con .config/yin_yang_dev.json = fixture
    let c = Config::load(&paths).unwrap();
    assert_eq!(c.mode, Mode::FollowSun);
    assert!(c.update_location);
    assert_eq!(c.plugins["vs code"].dark_theme.as_deref(), Some("Solarized Dark"));
    assert_eq!(c.plugins["wallpaper"].light_theme, None);
}
#[test] fn legacy_file_is_never_modified() { /* hash prima == hash dopo load+save */ }
#[test] fn missing_everything_gives_defaults() { /* mode Manual, dark_mode false, plugins vuoti */ }
#[test] fn newest_legacy_file_wins() { /* yin_yang.json più recente di yin_yang_dev.json */ }
#[test] fn old_version_gives_defaults() { /* version 2.2 -> default */ }
#[test] fn unknown_plugin_entries_survive_roundtrip() { /* la voce "firefox" della fixture resta identica dopo load+save */ }
#[test] fn save_is_atomic_and_roundtrips() { /* nessun *.tmp rimasto; load(save(c)) == c */ }#[test] fn accepts_both_time_formats() { /* ["07:00:00","20:00"] -> 07:00 e 20:00; save scrive "HH:MM" */ }
#[test] fn rejects_bad_time() { /* "25:99" -> Err */ }
#[test] fn fill_defaults_adds_only_missing_disabled() { /* plugin esistenti intatti */ }
```

- [ ] **Step 3: Eseguire `cargo test --test config`**

Expected: FAIL (tipi non definiti).

- [ ] **Step 4: Implementare `paths.rs` e `config.rs` secondo le interfacce**

Serde con modulo di (de)serializzazione per `NaiveTime`; scrittura atomica con `tempfile::NamedTempFile::persist`.

- [ ] **Step 5: Eseguire `cargo test --test config`**

Expected: PASS (10 test).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: scaffold crate, reference copy and config model"
```

---

### Task 2: Pianificazione, posizione e alba/tramonto

**Files:**
- Create: `src/schedule.rs`, `src/location.rs`, `tests/schedule.rs`

**Interfaces:**
- Consumes: `Config`, `Mode`, `Paths` (Task 1).
- Produces:
  - `schedule::should_be_dark(now: NaiveTime, light: NaiveTime, dark: NaiveTime) -> bool` (stessa logica di `reference/yin_yang/theme_switcher.py:23-29`).
  - `schedule::sun_times(lat: f64, lon: f64, date: NaiveDate) -> Option<(DateTime<Utc>, DateTime<Utc>)>` (alba, tramonto; `None` se il sole non sorge o non tramonta quel giorno).
  - `trait LocationSource { fn current(&self) -> Option<(f64, f64)>; }`; `IpInfoLocation` (GET `https://ipinfo.io/json`, campo `"loc": "lat,lon"`, timeout 5 s, ogni errore → `None`); `NoNetworkLocation` (restituisce sempre `None`, per l'host di Firefox e per `status`).
  - `location::resolve(cfg: &Config, paths: &Paths, src: &dyn LocationSource) -> anyhow::Result<(f64, f64)>`: se `update_location` interroga `src` e salva in `paths.state_dir/location.json`; se `src` non risponde usa il file salvato; senza nessuno dei due → `Err`. Le coordinate (0,0) valgono "non impostate" e sono sempre errore.
  - `schedule::effective_times(cfg: &Config, loc: &dyn LocationSource, paths: &Paths, date: NaiveDate, offset: FixedOffset) -> anyhow::Result<(NaiveTime, NaiveTime)>`: `Manual`/`Scheduled` → `cfg.times`; `FollowSun` → alba/tramonto locali, e se `sun_times` è `None` ripiega su `cfg.times` con `log::warn!`.

- [ ] **Step 1: Scrivere i test in `tests/schedule.rs`**

```rust
// should_be_dark (light, dark) = (07:00, 20:00)
assert!(!sbd(t(12,0), t(7,0), t(20,0)));
assert!(sbd(t(21,0), t(7,0), t(20,0)));
assert!(sbd(t(6,0),  t(7,0), t(20,0)));
assert!(!sbd(t(7,0), t(7,0), t(20,0)));   // confine: inizia il chiaro
assert!(sbd(t(20,0), t(7,0), t(20,0)));   // confine: inizia lo scuro
// attraversa la mezzanotte: chiaro 22:00, scuro 06:00
assert!(!sbd(t(23,0), t(22,0), t(6,0)));
assert!(!sbd(t(3,0),  t(22,0), t(6,0)));
assert!(sbd(t(12,0),  t(22,0), t(6,0)));
// light == dark (07:00, 07:00): non va in panico e dà lo stesso risultato per lo stesso input
// Valori misurati con il crate e confrontati con `suntime` (la libreria dell'originale), tolleranza 2 minuti:
// Roma (41.9028, 12.4964) 2026-06-21: alba 03:34:49Z, tramonto 18:48:38Z; 2026-12-21: alba 06:34:16Z, tramonto 15:41:40Z
// Auckland (-36.85, 174.76) 2026-10-29 con offset +13h: alba locale 06:19, tramonto locale 19:49 (l'alba cade nel 28 UTC)
// Honolulu (21.3, -157.86) 2026-10-29 con offset -10h: alba locale 06:32, tramonto locale 17:57 (il tramonto cade nel 30 UTC)
// Tromsø (69.65, 18.96) 2026-06-21 (sole di mezzanotte) e 2026-12-21 (notte polare) -> None
// Coordinates fuori range (lat 91.0) -> Err
// resolve: update_location=true, FakeLoc(None), nessun file salvato -> Err
// resolve: FakeLoc(Some((45.0,9.0))) poi FakeLoc(None) -> (45.0,9.0) dal file salvato
// resolve: update_location=false e coordinates=(0,0) -> Err
// effective_times: FollowSun a Tromsø -> uguale a cfg.times
```

- [ ] **Step 2: `cargo test --test schedule` → FAIL**

- [ ] **Step 3: Implementare i moduli**

Dipendenza verificata: `sunrise = "3.0"` (3.0.0, MIT, unica dipendenza `chrono`). API: `Coordinates::new(lat, lon) -> Option<Coordinates>` (`None` fuori range), `SolarDay::new(coord, date: NaiveDate)`, `SolarDay::event_time(SolarEvent::Sunrise | SolarEvent::Sunset) -> Option<DateTime<Utc>>` (`None` in giorno/notte polare). `sun_times` restituisce `Some` solo se **entrambi** gli eventi esistono. Il risultato può cadere nel giorno UTC precedente o successivo (Auckland 2026-10-29: alba il 28 alle 17:19Z): `effective_times` converte con `offset` e prende solo l'ora locale. `Coordinates::new` che fallisce → errore chiaro. `IpInfoLocation` con `ureq`.

- [ ] **Step 4: `cargo test --test schedule` → PASS**

- [ ] **Step 5: Commit** — `git commit -am "feat: schedule, sun times and location resolution"`

---

### Task 3: Framework dei plugin ed esecuzione

**Files:**
- Create: `src/plugin.rs`, `src/exec.rs`, `src/bus.rs`, `src/runner.rs`, `tests/support/mod.rs`, `tests/runner.rs`

**Interfaces:**
- Consumes: `Config`, `PluginConfig` (Task 1).
- Produces:
  - `exec::Exec: Send + Sync` con `fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<ExecOutput>` e `fn exists(&self, program: &str) -> bool` (ricerca nel `PATH`, senza eseguire); `ExecOutput { status: i32, stdout: String }`; `RealExec`.
  - `bus::Bus: Send + Sync` con `call_str(&self, dest: &str, path: &str, iface: &str, method: &str, arg: &str) -> Result<()>`, `call_bool(&self, dest, path, iface, method, arg: bool) -> Result<()>`, `introspect(&self, dest: &str, path: &str) -> Result<String>`, `service_names(&self) -> Result<Vec<String>>`, `notify(&self, summary: &str, body: &str) -> Result<()>`. `RealBus` si connette **pigramente** alla prima chiamata: senza sessione D-Bus ogni metodo restituisce `Err`, la costruzione non fallisce (`RealBus::session() -> RealBus`).
  - `plugin::Env { home: PathBuf, kde: bool, exec: Arc<dyn Exec>, bus: Arc<dyn Bus> }`.
  - `plugin::Variant { Light, Dark }`; `plugin::theme_for(cfg: &PluginConfig, v: Variant) -> anyhow::Result<&str>` (`Err` se il tema è `None` o vuoto).
  - `plugin::InputKind { Choice, Text, File }`; `ThemeChoice { id: String, label: String }`.
  - `trait Plugin: Send + Sync { fn key(&self) -> &'static str; fn name(&self) -> &'static str; fn kind(&self) -> InputKind; fn defaults(&self) -> (Option<&'static str>, Option<&'static str>); fn available(&self, env: &Env) -> bool; fn themes(&self, env: &Env) -> Vec<ThemeChoice>; fn apply(&self, env: &Env, variant: Variant, cfg: &PluginConfig) -> anyhow::Result<()>; fn describe(&self, variant: Variant, cfg: &PluginConfig) -> String { format!("{}: {}", self.key(), theme_for(cfg, variant).unwrap_or("-")) } fn is_notification(&self) -> bool { false } }`.
  - `plugin::registry() -> Vec<Box<dyn Plugin>>` nell'ordine: system, colors, gtk, icons, kvantum, wallpaper, vs code, onlyoffice, okular, konsole, custom, notification (inizialmente vuoto, riempito dai task successivi).
  - `runner::Outcome { Applied, Skipped(String), Failed(anyhow::Error) }`.
  - `runner::apply_all(env: &Env, cfg: &Config, plugins: &[Box<dyn Plugin>], variant: Variant, quiet: bool) -> Vec<(String, Outcome)>`: prima esegue `system` da solo (applica anche colori e icone di Plasma), poi tutti gli altri abilitati in parallelo (`std::thread::scope`, con `join()` esplicito per ciascun thread: un panico diventa `Failed`); disabilitato → non compare; abilitato ma non disponibile → `Skipped`; `quiet == true` salta i plugin con `is_notification()`; risultati nell'ordine del registro.
  - `runner::plan(cfg: &Config, plugins: &[Box<dyn Plugin>], variant: Variant, quiet: bool) -> Vec<String>`: le righe di `describe` dei plugin che `apply_all` eseguirebbe, senza eseguire niente.
  - `tests/support`: `FakeExec` (registra le chiamate; risposte per programma e `exists` configurabili; errore iniettabile), `FakeBus` (registra le chiamate; `service_names` configurabili; risposta di `introspect` per `(dest, path)` con righe `  <node name="1"/>` come in `reference/yin_yang/plugins/_plugin.py:279-294`; errore iniettabile).

- [ ] **Step 1: Scrivere `tests/runner.rs` con plugin finti**

```rust
#[test] fn failing_plugin_does_not_stop_others()      { /* A Err, B Ok -> entrambi eseguiti */ }
#[test] fn panicking_plugin_is_failed_not_propagated() { /* nessun panico verso l'esterno */ }
#[test] fn disabled_not_listed_unavailable_skipped()  { /* apply non chiamato; Skipped con motivo */ }
#[test] fn quiet_skips_notification_only()            { /* is_notification() == true saltato */ }
#[test] fn system_runs_before_all_others()            { /* tempo di fine di system < inizio degli altri */ }
#[test] fn results_follow_registry_order()            { /* anche con thread lenti */ }
#[test] fn plan_executes_nothing()                    { /* nessuna chiamata a apply/Exec/Bus; righe attese */ }
#[test] fn theme_for_rejects_none_and_empty()         { /* Err */ }
#[test] fn real_bus_without_session_errors_not_panics() { /* DBUS_SESSION_BUS_ADDRESS impostato a un socket inesistente -> Err */ }
```

- [ ] **Step 2: `cargo test --test runner` → FAIL**
- [ ] **Step 3: Implementare trait, `RealExec`, `RealBus` (zbus::blocking, connessione pigra; `introspect` = `org.freedesktop.DBus.Introspectable.Introspect`; `service_names` = `org.freedesktop.DBus.ListNames`; `notify` = `org.freedesktop.Notifications.Notify` con app `yinyang`, timeout 5000), `runner`, `support`**
- [ ] **Step 4: `cargo test --test runner` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: plugin framework, exec and bus abstractions, runner"`

---

### Task 4: Plugin a comando (system, colors, icons, kvantum)

**Files:**
- Create: `src/plugins/mod.rs`, `src/plugins/system.rs`, `src/plugins/colors.rs`, `src/plugins/icons.rs`, `src/plugins/kvantum.rs`, `tests/plugins_command.rs`
- Modify: `src/plugin.rs` (`registry()`)

**Interfaces:**
- Consumes: `Plugin`, `Env`, `theme_for`, `FakeExec` (Task 3).
- Produces: `System`, `Colors`, `Icons`, `Kvantum` (unit struct che implementano `Plugin`), tutti `InputKind::Choice`.

Comportamento (sorgente da portare e da cui copiare i default: `reference/yin_yang/plugins/system.py:136-195`, `colors.py`, `icons.py:95-132`, `kvantum.py`):
- `system`: `lookandfeeltool -a <theme>`; temi da `lookandfeeltool -l`, etichetta da `metadata.json` (`KPlugin.Name`, preferendo `Name[<locale>]` come in `system.py:128-133`), poi `metadata.desktop`, poi l'id, cercando in `/usr/share/plasma/look-and-feel/<id>` e `~/.local/share/plasma/look-and-feel/<id>`. Default `org.kde.breeze.desktop` / `org.kde.breezedark.desktop`.
- `colors`: `plasma-apply-colorscheme <theme>`; temi dalle sole righe ` * ` di `plasma-apply-colorscheme --list-schemes`, togliendo il suffisso `(...)` (l'intestazione è tradotta: non basta saltare la prima riga; `colors.py:19-27`).
- `icons`: `/usr/lib/plasma-changeicons <theme>`; temi come in `icons.py:103-128`: cartelle di `/usr/share/icons` e `~/.icons` con `index.theme` e `icon-theme.cache`, escludendo i cursori (nessuna chiave `Size`), etichetta `[Icon Theme] Name`; disponibile se il binario esiste.
- `kvantum`: `kvantummanager --set <theme>`; temi = nomi dei file `*.kvconfig` cercati ricorsivamente in `/usr/share/Kvantum` e `~/.config/Kvantum` (`kvantum.py:16-22`); disponibile se `env.exec.exists("kvantummanager")`.
- Se `env.kde == false`, `system`, `colors` e `icons` sono non disponibili.

- [ ] **Step 1: Scrivere i test in `tests/plugins_command.rs`**

```rust
// FakeExec registra le chiamate; home temporanea con cartelle tema finte
assert_eq!(fake.calls(), [("lookandfeeltool", vec!["-a", "org.kde.breezedark.desktop"])]);
assert_eq!(fake.calls(), [("plasma-apply-colorscheme", vec!["MacTahoeDark"])]);
assert_eq!(fake.calls(), [("/usr/lib/plasma-changeicons", vec!["breeze-dark"])]);
assert_eq!(fake.calls(), [("kvantummanager", vec!["--set", "MacTahoeDark"])]);
// themes(): output finto di `lookandfeeltool -l`; `--list-schemes` in italiano ("Hai i seguenti schemi di colori sul tuo sistema:" + righe " * Nome (current color scheme)")
//   -> id e label attesi, intestazione ignorata
// icons: cartella cursori (senza chiavi Size) esclusa
// kvantum: .kvconfig annidato trovato; exists("kvantummanager") == false -> available() == false
// kde=false -> system/colors/icons.available() == false
// tema None o "" -> apply() restituisce Err senza eseguire nulla
```

- [ ] **Step 2: `cargo test --test plugins_command` → FAIL**
- [ ] **Step 3: Implementare i quattro plugin e registrarli**
- [ ] **Step 4: `cargo test --test plugins_command` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: system, colors, icons and kvantum plugins"`

---

### Task 5: Plugin D-Bus (gtk, wallpaper, notification)

**Files:**
- Create: `src/plugins/gtk.rs`, `src/plugins/wallpaper.rs`, `src/plugins/notification.rs`, `tests/plugins_dbus.rs`
- Modify: `src/plugin.rs` (`registry()`)

**Interfaces:**
- Consumes: `Bus`, `FakeBus`, `FakeExec`, `Env`, `theme_for` (Task 3).
- Produces: `Gtk` (`Choice`), `Wallpaper` (`File`), `Notification` (`Text`, `is_notification() == true`).

Comportamento (sorgente: `reference/yin_yang/plugins/gtk.py:44-47,80-124`, `wallpaper.py:89-137`, `notify.py`):
- `gtk`: se `org.kde.GtkConfig` è tra i `service_names` → `call_str("org.kde.GtkConfig","/GtkConfig","org.kde.GtkConfig","setGtkTheme",theme)`; altrimenti, **solo se** `~/.config/xsettingsd/xsettingsd.conf` esiste, sostituisce la riga `Net/ThemeName` già presente, esegue `killall -HUP xsettingsd`, poi `gsettings set org.gnome.desktop.interface gtk-theme <theme>` e `color-scheme` come nell'originale. Temi = cartelle con `gtk-3.0` in `/usr/share/themes`, `/usr/local/share/themes`, `~/.themes`, `~/.local/share/themes`.
- `wallpaper`: `call_str("org.kde.plasmashell","/PlasmaShell","org.kde.PlasmaShell","evaluateScript",<script>)` con lo script copiato dall'originale. Il percorso entra nello script come URL `file://` con percentuale-codifica (`#` → `%23`, spazio → `%20`). File inesistente → `Err`.
- `notification`: `bus.notify("Theme changed", format!("Set the theme to {theme}"))`. I valori "Day"/"Night" sono solo testo.

- [ ] **Step 1: Scrivere i test in `tests/plugins_dbus.rs`**

```rust
// FakeBus con service_names = ["org.kde.GtkConfig"] -> una call_str setGtkTheme("MacTahoe-Dark")
// FakeBus senza GtkConfig, xsettingsd.conf presente -> riga Net/ThemeName sostituita, killall e gsettings chiamati
// FakeBus senza GtkConfig, xsettingsd.conf assente -> nessuna scrittura né gsettings (come l'originale)
// wallpaper "/tmp/x/#a b.png" (file creato) -> lo script passato contiene "file:///tmp/x/%23a%20b.png"
// wallpaper su file inesistente -> Err, nessuna call_str
// notification -> notify("Theme changed", "Set the theme to Night")
// FakeBus che restituisce Err -> apply() restituisce Err (non panico)
```

- [ ] **Step 2: `cargo test --test plugins_dbus` → FAIL**
- [ ] **Step 3: Implementare i plugin e registrarli**
- [ ] **Step 4: `cargo test --test plugins_dbus` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: gtk, wallpaper and notification plugins"`

---

### Task 6: Editor che preservano il formato (INI e JSONC)

**Files:**
- Create: `src/edit/mod.rs`, `src/edit/ini.rs`, `src/edit/jsonc.rs`, `tests/edit.rs`

**Interfaces:**
- Produces:
  - `edit::ini::set_value(text: &str, section: &str, key: &str, value: &str) -> String`: sostituisce `key=` o `key = ` nella sezione (preservando righe, commenti, spazi attorno a `=`, fine riga); se la chiave manca la aggiunge in fondo alla sezione; se la sezione manca la aggiunge in fondo al file.
  - `edit::ini::get_value(text: &str, section: &str, key: &str) -> Option<String>`.
  - `edit::ini::remove_key(text: &str, section: &str, key: &str) -> String` e `edit::ini::remove_section_if_empty(text: &str, section: &str) -> String`.
  - `edit::jsonc::set_top_level_string(text: &str, key: &str, value: &str) -> anyhow::Result<String>`: cambia solo il valore stringa della chiave di primo livello (la prima occorrenza); se manca la inserisce subito dopo la `{` iniziale, con la virgola corretta anche per `{}`; ignora le occorrenze dentro stringhe e commenti; `Err` se il testo non ha un oggetto di primo livello riconoscibile.
  - `edit::write_preserving(path: &Path, text: &str) -> anyhow::Result<()>`: risolve i link simbolici, scrive su un temporaneo nella stessa cartella con gli stessi permessi del file originale, poi `rename`.

- [ ] **Step 1: Scrivere i test in `tests/edit.rs`**

```rust
// ini: valore sostituito, commenti '#', righe vuote, ordine e CRLF identici; "Name = Dark" (con spazi) riconosciuto
// ini: chiave mancante aggiunta in fondo alla sezione giusta; sezione mancante aggiunta in fondo
// ini: stessa chiave in due sezioni diverse -> cambia solo quella richiesta
// ini: remove_key poi remove_section_if_empty: sezione [Document] vuota sparisce, altre intatte
// jsonc: input con // e /* */, virgola finale e indentazione a tab -> output identico tranne il valore
// jsonc: "workbench.colorTheme" dentro un commento // e dentro un'altra stringa -> ignorato
// jsonc: chiave duplicata -> cambia la prima di primo livello
// jsonc: chiave mancante -> inserita dopo '{' con indentazione e virgola giuste; su "{}" nessuna virgola in più
// jsonc: file vuoto o '[]' -> Err
// jsonc: valore con virgolette/backslash -> escape JSON corretto
// write_preserving: file 0644 resta 0644; link simbolico resta link e il bersaglio è aggiornato
```

- [ ] **Step 2: `cargo test --test edit` → FAIL**
- [ ] **Step 3: Implementare gli editor**

Per JSONC: scansione carattere per carattere con stati (in stringa, commento di riga, commento a blocco) che tiene traccia della profondità delle graffe; non serve un parser completo.

- [ ] **Step 4: `cargo test --test edit` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: format-preserving ini and jsonc editors"`

---

### Task 7: Plugin su file (vs code, onlyoffice, okular)

**Files:**
- Create: `src/plugins/vscode.rs`, `src/plugins/onlyoffice.rs`, `src/plugins/okular.rs`, `tests/plugins_files.rs`
- Modify: `src/plugin.rs` (`registry()`)

**Interfaces:**
- Consumes: `edit::*` (Task 6), `Bus`, `Env`, `theme_for` (Task 3).
- Produces: `Vscode` (key `"vs code"`, `Choice`), `OnlyOffice` (key `"onlyoffice"`, `Choice`), `Okular` (key `"okular"`, `Choice`).

Comportamento (sorgente: `reference/yin_yang/plugins/vscode.py`, `only_office.py`, `okular.py`):
- `vscode`: per ognuno dei `settings.json` esistenti (VSCodium, Code - OSS, Code, Code - Insiders sotto `~/.config/<nome>/User/`, più le varianti Flatpak sotto `~/.var/app/...` come in `vscode.py:66-77`) applica `jsonc::set_top_level_string(text, "workbench.colorTheme", theme)` e scrive con `write_preserving`. File illeggibile o `Err` dell'editor → non lo tocca e restituisce `Err`. Default `Default Light Modern` / `Default Dark Modern`. Temi: `package.json` di **tutte** le cartelle di `extension_paths` (`vscode.py:13-36`, incluse `/usr/lib/code/extensions`, `/opt/visual-studio-code/...`, snap e Flatpak), identificativo = `id` se presente, altrimenti `label` (`vscode.py:60`). Disponibile se l'elenco dei temi non è vuoto.
- `onlyoffice`: `ini::set_value(text, "General", "UITheme", theme)` su `~/.config/onlyoffice/DesktopEditors.conf` e sulla variante Flatpak. Default `theme-light` / `theme-dark`; elenco fisso dei temi come in `only_office.py:23-29`.
- `okular`: (a) per ogni servizio `org.kde.okular-*` tra i `service_names` chiama `call_bool(dest,"/okular","org.kde.okular","slotSetChangeColors", dark)` (cambio dal vivo, `okular.py:28-44`); (b) aggiorna `~/.config/okularpartrc`: scuro → `[Document] ChangeColors=true`, chiaro → `remove_key` poi `remove_section_if_empty("Document")` (`okular.py:49-58`); (c) scrive `RenderMode` come in `okular.py:82-106`. Default `""` / `"InvertLightness"`; l'elenco dei temi include la voce vuota ("disattivato").

- [ ] **Step 1: Scrivere i test in `tests/plugins_files.rs`**

```rust
// vscode: settings.json con commenti e virgola finale -> solo "workbench.colorTheme" cambia (confronto byte per byte)
// vscode: file corrotto (non JSON) -> Err e file identico a prima
// vscode: due editor installati -> entrambi aggiornati; nessun editor -> available() == false
// vscode: temi da /usr/lib/code/extensions/theme-solarized-dark/package.json (cartella finta sotto home/root di prova) -> compare "Solarized Dark"
// onlyoffice: [General] UITheme=theme-light -> theme-dark, resto invariato
// okular scuro: FakeBus con service_names ["org.kde.okular-4242"] -> call_bool(..., "slotSetChangeColors", true); okularpartrc ha ChangeColors=true
// okular chiaro: call_bool(..., false); ChangeColors rimossa e [Document] sparisce se vuota
// okular senza istanze in esecuzione -> Ok, file comunque aggiornato
// nessun file di config -> available() == false, apply() non crea file
```

- [ ] **Step 2: `cargo test --test plugins_files` → FAIL**
- [ ] **Step 3: Implementare i tre plugin e registrarli**
- [ ] **Step 4: `cargo test --test plugins_files` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: vscode, onlyoffice and okular plugins"`

---

### Task 8: Plugin Konsole

**Files:**
- Create: `src/plugins/konsole.rs`, `tests/plugin_konsole.rs`
- Modify: `src/plugin.rs` (`registry()`)

**Interfaces:**
- Consumes: `edit::*`, `Bus`, `Env`, `theme_for` (Task 3, 6).
- Produces: `Konsole` (key `"konsole"`, `Choice`); default `BlackOnWhite` / `Breeze`; temi = file `*.colorscheme` in `/usr/share/konsole` e `~/.local/share/konsole`, etichetta = `Description`; disponibile se esiste una di quelle cartelle (come `konsole.py:136-137`).

Comportamento (sorgente: `reference/yin_yang/plugins/konsole.py:39-55,140-182,209-269`): `apply` riceve `PluginConfig` con **entrambi** i temi. Crea (se mancano) i profili `Light` e `Dark` in `~/.local/share/konsole`: base = profilo indicato da `[Desktop Entry] DefaultProfile=` di `~/.config/konsolerc`; se quel file non esiste usa il primo `.profile` presente, altrimenti crea `Default.profile` con il contenuto minimo dell'originale. Imposta `[Appearance] ColorScheme=<tema chiaro>` in `Light` e `<tema scuro>` in `Dark`; imposta `DefaultProfile` di konsolerc sul profilo della variante; per ogni servizio D-Bus il cui nome (prima del `-`) è `org.kde.konsole`, `org.kde.yakuake`, `org.kde.dolphin`, `org.kde.kate` cambia il profilo delle sessioni (`introspect` + `setProfile`) e il profilo predefinito delle finestre (`setDefaultProfile`). Nessuna istanza in esecuzione non è un errore.

- [ ] **Step 1: Scrivere i test in `tests/plugin_konsole.rs`**

```rust
// konsolerc [Desktop Entry] DefaultProfile=Mio.profile -> creati Light.profile/Dark.profile con ColorScheme chiaro/scuro
// apply(Dark) -> DefaultProfile=Dark.profile in konsolerc
// DefaultProfile punta a un file inesistente -> si usa il primo .profile; nessun .profile -> creato Default.profile
// FakeBus con service_names ["org.kde.konsole-1234"] e introspect con due nodi -> setProfile("Dark") per ogni sessione trovata
// FakeBus senza servizi -> apply() == Ok e file comunque aggiornati
// cartelle konsole assenti -> available() == false, nessun file creato
```

- [ ] **Step 2: `cargo test --test plugin_konsole` → FAIL**
- [ ] **Step 3: Implementare `Konsole` e registrarlo**
- [ ] **Step 4: `cargo test --test plugin_konsole` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: konsole plugin"`

---

### Task 9: Plugin custom

**Files:**
- Create: `src/plugins/custom.rs`, `tests/plugin_custom.rs`
- Modify: `src/plugin.rs` (`registry()`)

**Interfaces:**
- Consumes: `Exec`, `theme_for` (Task 3).
- Produces: `Custom` (key `"custom"`, `Text`): esegue `sh -c <theme>` tramite `Exec` (il "tema" è il comando da eseguire per la variante); comando vuoto o `None` → `Err` senza eseguire nulla; sempre `available`; default vuoti.

Firefox, Zen e i loro fork non hanno un plugin: il tema predefinito (`default-theme@mozilla.org`) segue da solo il tema di sistema tramite il portale `org.freedesktop.appearance`, quindi cambia quando cambia quello di Plasma. L'eventuale voce `"firefox"` di una config importata resta nel file senza essere usata (test `unknown_plugin_entries_survive_roundtrip`, Task 1).

- [ ] **Step 1: Scrivere i test**

```rust
// custom: apply(Dark, light="a", dark="notify-send ciao") -> FakeExec vede ("sh", ["-c", "notify-send ciao"])
// custom: comando vuoto o None -> Err e nessuna chiamata a Exec
// custom: exit status diverso da 0 -> Err con lo stato
```

- [ ] **Step 2: `cargo test --test plugin_custom` -> FAIL**
- [ ] **Step 3: Implementare `Custom` e registrarlo**
- [ ] **Step 4: `cargo test --test plugin_custom` -> PASS**
- [ ] **Step 5: Commit** - `git commit -am "feat: custom plugin"`

---

### Task 10: Timer systemd

**Files:**
- Create: `src/systemd.rs`, `tests/systemd.rs`

**Interfaces:**
- Consumes: `Config`, `Mode`, `Paths`, `Exec` (Task 1, 3).
- Produces:
  - `systemd::render_service(exe: &Path) -> String` → `Type=oneshot`, `After=graphical-session.target`, `ExecStart=<exe> apply --auto`.
  - `systemd::render_timer(light: NaiveTime, dark: NaiveTime, boot_offset: u32) -> String` → `OnCalendar=HH:MM:SS` ×2 (senza frazioni di secondo), `OnStartupSec=<n>`, `Persistent=true`, `WantedBy=timers.target`.
  - `systemd::refresh(cfg: &Config, paths: &Paths, exec: &dyn Exec, exe: &Path, times: Option<(NaiveTime, NaiveTime)>) -> anyhow::Result<()>`: `Manual` → `systemctl --user disable --now yinyang.timer`; altrimenti, se `times` è `Some`, scrive service e timer solo se il contenuto cambia e, in tal caso, `systemctl --user daemon-reload`; poi **sempre** `systemctl --user enable --now yinyang.timer`. Con `times == None` (posizione non disponibile) i file esistenti restano come sono.

- [ ] **Step 1: Scrivere i test in `tests/systemd.rs`**

```rust
// render_timer(07:30:36, 18:11:24, 10) contiene "OnCalendar=07:30:36", "OnCalendar=18:11:24", "OnStartupSec=10", "Persistent=true"
// render_service(Path::new("/home/u/.local/bin/yinyang")) contiene "ExecStart=/home/u/.local/bin/yinyang apply --auto" e "After=graphical-session.target"
// refresh Scheduled, primo run: file scritti; FakeExec vede daemon-reload poi enable --now
// refresh con stessi orari: file non riscritti (mtime invariato), NESSUN daemon-reload, ma enable --now chiamato
// refresh con orari diversi (alba di domani): file riscritti e daemon-reload chiamato
// refresh con times None: file invariati, enable --now chiamato
// refresh con Manual: solo `disable --now`, nessun file creato
```

- [ ] **Step 2: `cargo test --test systemd` → FAIL**
- [ ] **Step 3: Implementare `systemd.rs`**
- [ ] **Step 4: `cargo test --test systemd` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: systemd user timer management"`

---

### Task 11: CLI

**Files:**
- Modify: `src/main.rs`; Create: `src/cli.rs`, `src/app.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: tutti i moduli precedenti.
- Produces — sottocomandi di `yinyang` (parsing con `clap` derive):
  - `apply [--auto | --dark | --light] [--force] [--dry-run]`: prende il lock, carica la config, calcola la variante (`--auto` = `should_be_dark` su `effective_times`; se la posizione manca → errore, exit 1, niente plugin, `dark_mode` invariato), **aggiorna sempre il timer** (`systemd::refresh` con gli orari effettivi) anche se il tema non cambia, poi: se la variante è uguale a `dark_mode` e non c'è `--force` esce; altrimenti `runner::apply_all` (con `quiet = force`), salva `dark_mode` solo se nessun esito è `Failed`, stampa gli esiti su stderr, exit 1 se qualche plugin ha fallito. Con `--auto`, se un plugin fallisce, invia `bus.notify("Yin-Yang", <errore>)`. `--dry-run`: stampa le righe di `runner::plan` e non esegue né scrive nulla (nemmeno `systemctl`).
  - `toggle`: porta `mode` a `Manual`, inverte `dark_mode`, applica, `systemd::refresh`.
  - `status`: stampa JSON `{ "dark_mode", "mode", "times": ["HH:MM","HH:MM"] | null, "location": [lat, lon] | null }` (con `NoNetworkLocation`, quindi usa le coordinate salvate).
  - `plugins --json`: `[{"key","name","kind":"choice|text|file","available","themes":[{"id","label"}]}]` nell'ordine del registro. Funziona anche senza sessione D-Bus.
  - `config show`: stampa la config (con `fill_defaults`) come JSON.
  - `config save`: prende il lock, legge JSON da stdin, deserializza e valida (`Err` + exit 2 se non valido); **mantiene il `dark_mode` presente nel file** (ignora quello ricevuto), salva, `systemd::refresh`.
  - `timer refresh`: lock, poi solo `systemd::refresh`.
- `app.rs` costruisce `Env` reale e `Paths::from_env()`; i sottocomandi sono funzioni che ricevono `Env`, `Paths` e un `LocationSource`, così `tests/cli.rs` li chiama con i finti.

- [ ] **Step 1: Scrivere i test in `tests/cli.rs`**

```rust
// fixture con dark_mode portato a false; home finta con cartella estensioni VS Code, settings.json e FakeExec che risponde a `kvantummanager`
// apply --dark (plugin abilitati: system, colors, gtk, kvantum, vs code, notification):
//   chiamate attese: lookandfeeltool -a com.github.vinceliuice.MacTahoe-Dark PRIMA delle altre,
//   plasma-apply-colorscheme MacTahoeDark, kvantummanager --set MacTahoeDark, setGtkTheme("MacTahoe-Dark"), notify;
//   settings.json aggiornato; config salvata con dark_mode == true
// apply --dark due volte -> la seconda non esegue plugin; con --force sì, e senza notification
// apply con un plugin in errore -> exit 1, gli altri eseguiti, dark_mode NON salvato
// apply --auto senza cambio di tema con orari di domani diversi -> timer rinnovato comunque (file riscritti, enable --now)
// apply --auto con update_location=true e nessuna posizione -> exit 1, nessun plugin chiamato, dark_mode invariato
// apply --auto con un plugin in errore -> notify("Yin-Yang", ...) chiamato
// apply --dry-run -> nessuna chiamata a Exec/Bus, nessuna scrittura (config, unit, file utente), righe plan stampate
// toggle -> mode == Manual e dark_mode invertito
// plugins --json con RealBus senza sessione -> 12 elementi nell'ordine del registro, nessun panico
// config save con JSON non valido -> exit 2, file invariato; valido -> file aggiornato, dark_mode del file conservato, timer refresh chiamato
// due `apply` concorrenti (thread) -> il secondo attende il lock, rilegge la config; file di config sempre JSON valido
```

- [ ] **Step 2: `cargo test --test cli` → FAIL**
- [ ] **Step 3: Implementare `cli.rs`, `app.rs`, `main.rs`**

Logging con `log` + `env_logger` su stderr (journald lo cattura per il service); `RUST_LOG=debug` per i dettagli.

- [ ] **Step 4: `cargo test` → PASS (tutta la suite)**
- [ ] **Step 5: Commit** — `git commit -am "feat: yinyang CLI"`

---

### Task 12: GUI Qt6 — client del nucleo e finestra principale

**Files:**
- Create: `gui/CMakeLists.txt`, `gui/main.cpp`, `gui/coreclient.h/.cpp` (`CoreClient`), `gui/mainwindow.h/.cpp`, `gui/main_window.ui` (partendo da `reference/designer/main_window.ui`), `gui/tests/tst_coreclient.cpp`, `gui/tests/tst_mainwindow.cpp`, `gui/tests/fake_yinyang.sh`

**Interfaces:**
- Consumes: la CLI del Task 11 (formati JSON di `plugins --json`, `config show`, `config save`, `status`).
- Produces:
  - `class CoreClient : public QObject` con `CoreClient(QString program)`; sincrone (timeout 10 s) `QJsonArray plugins()`, `QJsonObject config()`, `QJsonObject status()`, `bool save(const QJsonObject&, QString *error)`; asincrone (non bloccano la finestra) `void apply(bool dark, bool force)`, `void toggle()` con segnale `finished(bool ok, QString error)`.
  - `class MainWindow : public QMainWindow` con `MainWindow(CoreClient *client)`: widget del `.ui` (`btn_enable`, `btn_schedule`, `btn_sun`, `inp_time_light`, `inp_time_dark`, `inp_latitude`, `inp_longitude`, `btn_location`, `button_light`, `button_dark`, `toggle_notification`, `bootOffset`, `label_active`, `btn_box`). `btn_box` ha **Apply, Cancel, RestoreDefaults** (come `main_window.ui:424-426`): Apply = `save` + `apply(auto, force=true)` (`main_window_connector.py:263-280`); Cancel = ricarica dalla config; RestoreDefaults = valori predefiniti nei widget (si salvano con Apply). In modalità "sunset to sunrise" gli orari e la posizione mostrati vengono da `status()`. Per ogni plugin disponibile crea un `QGroupBox` checkable (`group<Name>`) con due `QComboBox` (`choice`), due `QLineEdit` (`text`) o due `QPushButton` + `QFileDialog` (`file`); se il valore salvato non è tra i temi elencati, viene **aggiunto come voce** (mai ripiegare sulla prima voce); `notification` è legata a `toggle_notification`. Chiudere con modifiche non salvate chiede conferma.

- [ ] **Step 1: Scrivere i test QtTest**

```cpp
// fake_yinyang.sh: risponde a "plugins --json", "config show", "status", "config save" (scrive stdin su file), "apply ..."
void CoreClientTest::parsesPlugins()       { QCOMPARE(client.plugins().size(), 3); QCOMPARE(client.plugins()[0].toObject()["key"].toString(), "system"); }
void CoreClientTest::saveWritesStdin()     { QVERIFY(client.save(cfg, &err)); /* contenuto del file == cfg */ }
void CoreClientTest::failureReported()     { /* programma che esce con 2 -> save() false e err non vuoto */ }
void CoreClientTest::applyIsAsync()        { /* fake con sleep: apply() ritorna subito, finished arriva dopo */ }
void MainWindowTest::buildsPluginGroups()  { /* QT_QPA_PLATFORM=offscreen: "groupSystem" con 2 QComboBox */ }
void MainWindowTest::applyButtonSavesAndApplies() { /* click Apply: config salvata con mode "sunset to sunrise" e poi "apply --auto --force" */ }
void MainWindowTest::restoreDefaultsButton() { /* RestoreDefaults presente e imposta i default nei widget */ }
void MainWindowTest::unavailablePluginHidden() { /* available:false -> gruppo non visibile */ }
void MainWindowTest::unknownCurrentThemeKept() { /* tema salvato "Solarized Dark" non in elenco -> voce aggiunta e selezionata */ }
void MainWindowTest::sunModeShowsComputedTimes() { /* status().times mostrati nei QTimeEdit */ }
```

- [ ] **Step 2: Configurare e compilare: `cmake -S gui -B gui/build -DCMAKE_BUILD_TYPE=Release && cmake --build gui/build` → i test falliscono o non compilano**
- [ ] **Step 3: Implementare `CoreClient`, `MainWindow` e `main.cpp` (argomento `--core <percorso>`, predefinito `yinyang` nel `PATH`)**

`main.cpp` crea `QApplication` con `setDesktopFileName("yinyang-gui")`; nessun `setStyle`, così vale il tema di KDE.

- [ ] **Step 4: `QT_QPA_PLATFORM=offscreen ctest --test-dir gui/build --output-on-failure` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: Qt6 GUI client and main window"`

---

### Task 13: GUI — vassoio, traduzioni, voce di menu

**Files:**
- Create: `gui/tray.h/.cpp`, `gui/yinyang-gui.desktop`, `gui/tests/tst_tray.cpp`
- Modify: `gui/CMakeLists.txt`, `gui/main.cpp`, `gui/mainwindow.cpp` (`tr()` con contesti coerenti con `reference/resources/translations/*.ts`)
- Copy: `reference/resources/translations/yin_yang.*.ts` → `gui/translations/`, `reference/resources/icon.svg` → `gui/icon.svg`

**Interfaces:**
- Consumes: `CoreClient`, `MainWindow` (Task 12).
- Produces: `class Tray : public QObject` con `Tray(CoreClient*, MainWindow*)`: `QSystemTrayIcon` con menu "Open Yin Yang", "Toggle theme", "Quit"; clic sinistro mostra la finestra; clic centrale e "Toggle theme" invertono solo il tema con `apply(!dark, force=true)` senza cambiare `mode` (come `__main__.py:66-70,143-144` dell'originale; il `toggle` della CLI, che passa a Manual, resta per `--toggle`). Opzione `--minimized`. Se il vassoio non è disponibile, chiudere la finestra esce. Traduzioni compilate con `qt_add_translations` (disponibili: de_DE, nl_NL, zh_CN, zh_TW) e caricate per la locale di sistema; senza traduzione resta l'inglese.

- [ ] **Step 1: Scrivere `tst_tray.cpp`**

```cpp
void TrayTest::menuHasThreeActions()     { QCOMPARE(tray.menu()->actions().size(), 3); }
void TrayTest::toggleActionInvertsOnly() { tray.menu()->actions()[1]->trigger(); /* fake registra "apply --dark|--light --force", mai "toggle" */ }
void TrayTest::middleClickSameAsToggle() { tray.onActivated(QSystemTrayIcon::MiddleClick); }
void TrayTest::germanTranslationLoads()  { /* QLocale::German: il .qm si carica e almeno una stringa differisce dall'originale */ }
```

- [ ] **Step 2: `ctest` → FAIL**
- [ ] **Step 3: Implementare `Tray`, `--minimized`, traduzioni e il file `.desktop` (`Exec=yinyang-gui`, `Icon=yinyang`, `Categories=Settings;Qt;`)**
- [ ] **Step 4: `QT_QPA_PLATFORM=offscreen ctest --test-dir gui/build --output-on-failure` → PASS**
- [ ] **Step 5: Commit** — `git commit -am "feat: tray icon, translations and desktop entry"`

---

### Task 14: Installazione e passaggio dal vecchio programma

**Files:**
- Create: `install.sh`, `docs/CUTOVER.md`

**Interfaces:**
- Consumes: i binari dei Task 11-13.
- Produces: `./install.sh` (con `PATH` pulito) che esegue `cargo build --release`, `cmake -S gui -B gui/build -DCMAKE_BUILD_TYPE=Release && cmake --build gui/build`, e installa `yinyang` e `yinyang-gui` in `~/.local/bin`, il `.desktop` in `~/.local/share/applications`, l'icona in `~/.local/share/icons/hicolor/scalable/apps/yinyang.svg`. **Non** attiva alcun timer.

- [ ] **Step 1: Scrivere `install.sh` (`set -euo pipefail`, nessun `sudo`, `PATH` fissato all'inizio dello script)**
- [ ] **Step 2: Eseguire l'installazione con `PATH` pulito**

Run: `env -i HOME="$HOME" PATH="/usr/bin:/bin:$HOME/.cargo/bin" ./install.sh && ldd ~/.local/bin/yinyang`
Expected: installazione senza errori; `ldd` elenca solo librerie di sistema (`libc`, `libgcc_s`, `libm`), nessuna `python`.

- [ ] **Step 3: Verifica a secco con la configurazione reale**

Run: `yinyang plugins --json | jq 'map({key,available})'` e `yinyang apply --dark --force --dry-run`
Expected: 12 plugin elencati; le righe stampate coincidono con ciò che farebbe il vecchio programma (`system: com.github.vinceliuice.MacTahoe-Dark`, `colors: MacTahoeDark`, `kvantum: MacTahoe`/`MacTahoeDark`, ecc.); nessun file toccato (`stat` prima e dopo su `settings.json` e sulla config).

- [ ] **Step 4: Passaggio (chiedere conferma all'utente prima di eseguire)**

Run: `systemctl --user disable --now yin_yang.timer && yinyang timer refresh && systemctl --user list-timers yinyang.timer`
Expected: il vecchio timer è disattivato; `yinyang.timer` compare con la prossima esecuzione.

- [ ] **Step 5: Prova reale su Plasma**

Run: `yinyang apply --light --force; sleep 5; yinyang apply --dark --force; journalctl --user -u yinyang.service -n 20 --no-pager`
Expected: il desktop cambia davvero tema; nessun errore. Zen e Firefox (tema predefinito) passano da soli a chiaro/scuro seguendo Plasma; controllarlo a occhio. Poi `yinyang-gui` si apre con lo stile di Plasma, mostra gli stessi valori del vecchio programma e Apply aggiorna `~/.config/yinyang.json`.

- [ ] **Step 6: Scrivere `docs/CUTOVER.md` e commit**

Contenuto obbligatorio: come tornare indietro (`systemctl --user disable --now yinyang.timer && systemctl --user enable --now yin_yang.timer`); **avviso** che aprire la vecchia GUI riattiva `yin_yang.timer` (`reference/yin_yang/daemon_handler.py`) e che resta `~/.local/share/applications/yin_yang.desktop`; la disinstallazione del pacchetto pacman `yin-yang` resta una scelta dell'utente.

```bash
git add -A && git commit -m "feat: installer and cutover notes"
```
