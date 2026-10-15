# Disk Space TUI

[![CI](https://github.com/danielegabrovec/disk-space-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/danielegabrovec/disk-space-tui/actions/workflows/ci.yml)
[![Licenza: MIT](https://img.shields.io/badge/licenza-MIT-blue.svg)](LICENSE)
![Piattaforme](https://img.shields.io/badge/piattaforme-Windows%20%7C%20Linux%20%7C%20macOS-informational)

**Un programma da terminale, veloce, per vedere cosa occupa il disco e liberare spazio senza rompere niente.**

Gli indichi un disco o una cartella: la analizza in parallelo (un disco da 1,5 TB con 12 milioni
di file richiede circa 20 secondi), mostra le cartelle e i file più grandi, ti dice **se ogni voce
si può eliminare e perché**, e raccoglie in un'unica lista di pulizia i soliti mangia-spazio
(temporanei, cache, cestino, `node_modules`, `target`, `.next`, `bin/obj`…).

Funziona su **Windows, Linux e macOS**, con tastiera e mouse. L'interfaccia è in **italiano o
inglese** (scelta dalla lingua del sistema). 🇬🇧 [README in English](README.md)

```text
╭ Disk Space TUI ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ C:\  ██████░░░░░░░░░░░░░░░░░░  24,4%  liberi   2,81 TB  su 3,72 TB  · NTFS                                                               │
│ G:\  ███████░░░░░░░░░░░░░░░░░  28,2%  liberi   2,67 TB  su 3,72 TB  · FAT32                                                              │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
  1  Esplora   2  File grandi (13)   3  Pulizia (10,1 GB)
 Percorso: C:\demo › Projects   (clic su un pezzo del percorso per tornarci)
╭ 2 voci · ordinate per dimensione ──────────────────────────────────────────────────────────╮╭ Dettagli ──────────────────────────────────╮
│▶    1,76 GB ███████░░░░░  54,5%  ▸ web-shop                                                ││web-shop  1,76 GB                           │
│     1,46 GB █████░░░░░░░  45,5%  ▸ rust-cli  ± repository git                              ││Cartella · 3 file                           │
│                                                                                            ││Dimensione     1,76 GB                      │
│                                                                                            ││Della cartella 54,5%                        │
│                                                                                            ││Del disco      0,0%                         │
│                                                                                            ││ ? Nessuna informazione                     │
│                                                                                            ││Non so cosa sia: controlla tu cosa contiene │
│                                                                                            ││prima di eliminarlo. Il Cestino ti permette │
│                                                                                            ││di ripensarci.                              │
│                                                                                            ││ Invio  apri                                │
│                                                                                            ││ Spazio  tutte le azioni                    │
│                                                                                            ││ Canc  Cestino                              │
╰────────────────────────────────────────────────────── in questa cartella: 3,22 GB · 5 file ╯╰────────────────────────────────────────────╯
 ✓ Analisi completata: 15,4 GB in 13 file · 0.0 s · 3,22 GB di file di compilazione nella scheda 3 Pulizia
  Invio  apri   ←  indietro   Spazio  azioni   Canc  Cestino   Maiusc+Canc  elimina   /  cerca   S  ordina   F5  aggiorna
```

## Indice

- [Cosa fa](#cosa-fa)
- [Installazione](#installazione)
- [Uso](#uso)
- [Tastiera e mouse](#tastiera-e-mouse)
- [Le tre schede](#le-tre-schede)
- [Sicurezza](#sicurezza)
- [Cosa si pulisce, per sistema](#cosa-si-pulisce-per-sistema)
- [Riga di comando](#riga-di-comando)
- [Come si misurano le dimensioni](#come-si-misurano-le-dimensioni)
- [Note per sistema e problemi frequenti](#note-per-sistema-e-problemi-frequenti)
- [Compilare dai sorgenti](#compilare-dai-sorgenti)
- [Struttura del progetto](#struttura-del-progetto)
- [Contribuire](#contribuire) · [Licenza](#licenza)

## Cosa fa

- **Analisi parallela veloce** di un disco intero o di una cartella, con avanzamento, cartella in
  lettura e percentuale stimata quando analizzi un disco intero.
- **Esplora** le cartelle ordinate per dimensione, con barre, percentuali, percorso cliccabile,
  ricerca (`/`) e ordinamento per nome o dimensione.
- **«Si può eliminare?»** — il pannello Dettagli e le etichette colorate (*sistema*,
  *rigenerabile*, *pulibile*, *repository git*, *attenzione*) spiegano ogni voce prima di toccarla.
- **File grandi**: i 500 file più grandi dell'analisi, con salto alla loro cartella.
- **Pulizia** divisa per rischio: *sicuro* (temporanei, cestino, dump dei crash…), *cache* (npm,
  pip, cargo, NuGet, Gradle, browser…), *file di compilazione dei progetti* trovati dall'analisi,
  *con attenzione* e *da gestire con la loro app* (Ollama, LM Studio, Docker… mostrati solo per
  sapere quanto pesano).
- **Sicuro per costruzione**: le cartelle di sistema e quelle principali del profilo non si possono
  eliminare dal programma, i repository git non vengono mai toccati dalle pulizie automatiche,
  l'eliminazione va nel cestino per impostazione predefinita e quella definitiva chiede conferma
  con **Annulla** come pulsante predefinito.
- **Mouse**: clic, doppio clic, rotella, tasto destro per il menu delle azioni; schede, percorso,
  caselle e pulsanti cliccabili.
- **Rapporto testuale** e comandi per gli script (`--report`, `--artifacts`, `--delete-list`,
  `--clean-temp`) con lo stesso motore e le stesse protezioni.
- Un solo eseguibile, senza runtime né configurazione.

## Installazione

### Eseguibili pronti

Scarica l'archivio per il tuo sistema dalla pagina
[Releases](https://github.com/danielegabrovec/disk-space-tui/releases), estrailo e avvia
`disk-space-tui` (`disk-space-tui.exe` su Windows).

| Sistema | File |
|---|---|
| Windows 10/11 (x64) | `disk-space-tui-x86_64-pc-windows-msvc.zip` |
| Linux (x64) | `disk-space-tui-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Apple Silicon) | `disk-space-tui-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `disk-space-tui-x86_64-apple-darwin.tar.gz` |

Su macOS, se Gatekeeper blocca l'eseguibile scaricato:
`xattr -d com.apple.quarantine ./disk-space-tui`.

### Con Cargo

```sh
cargo install --git https://github.com/danielegabrovec/disk-space-tui
```

Serve Rust 1.88 o successivo ([rustup.rs](https://rustup.rs)).

## Uso

```sh
disk-space-tui              # interfaccia interattiva
disk-space-tui --lang en    # forza l'inglese (oppure: --lang it)
```

1. **Scegli cosa analizzare** — un disco, il profilo utente, Desktop, Documenti, Download… oppure
   premi `G` e scrivi/incolla un percorso qualsiasi (`~` funziona su Linux/macOS).
2. **Esplora** le cartelle più grandi. Il pannello **Dettagli** a destra dice cos'è una voce e se
   si può eliminare.
3. **Pulisci** nella scheda 3: seleziona con `Spazio` (o `A` per tutte le voci sicure) e premi `Invio`.

Con un terminale largo almeno 110 colonne il pannello Dettagli sta a lato, altrimenti in basso.
Funzionano Windows Terminal, iTerm2, GNOME Terminal, Konsole, Alacritty, WezTerm e kitty; il
mouse richiede un terminale che lo supporti (tutti quelli citati).

La lingua viene dal sistema (`it*` → italiano, altrimenti inglese). Si forza con `--lang it|en` o
con la variabile d'ambiente `DISK_SPACE_TUI_LANG=it|en`.

## Tastiera e mouse

La barra in fondo mostra sempre i tasti validi in quel momento; `?` apre l'aiuto completo.

| Dove | Tasti |
|---|---|
| Ovunque | `Tab` / `1` `2` `3` schede · `↑` `↓` `PagSu` `PagGiù` `Home` `Fine` muovi · `G` scrivi un percorso · `?` aiuto · `Q` esci |
| Esplora | `Invio` / `→` apri · `←` / `Backspace` / `Esc` indietro · `Spazio` **menu delle azioni** · `Canc` sposta nel cestino · `Maiusc+Canc` elimina definitivamente · `/` cerca · `S` ordina · `F5` rifai l'analisi · `N` nuova analisi · `O` mostra nel gestore file |
| File grandi | `Invio` azioni · `T` mostra nella cartella · `Canc` / `Maiusc+Canc` · `O` gestore file |
| Pulizia | `Spazio` seleziona · `A` tutte le sicure · `X` nessuna · `Invio` pulisci · `M` azioni · `N` analizza la cartella · `F5` rimisura |
| Finestre | `S`/`Y` sì · `N` no · `←` `→` cambia pulsante · `Invio` il pulsante evidenziato · `Esc` annulla |
| Mouse | clic seleziona · doppio clic apre · rotella scorre · **tasto destro apre il menu delle azioni** · schede, pezzi del percorso, caselle e pulsanti sono cliccabili |

`d` / `D` funzionano al posto di `Canc` / `Maiusc+Canc` (comodo sui portatili e sui Mac senza tasto Canc).

Il **menu delle azioni** (`Spazio` o tasto destro) elenca tutto ciò che si può fare sulla voce
selezionata, ognuna con la sua scorciatoia: apri, analizza solo questa cartella, mostra nel
gestore file, copia il percorso, sposta nel cestino, elimina definitivamente. Le azioni non
disponibili sono grigie e dicono perché.

```text
╭ web-shop ──────────────────────────────────────────────────────╮
│ Cosa vuoi fare?                                                │
│                                                                │
│ ▶ [Invio]  Apri la cartella                                    │
│   [N]      Analizza solo questa cartella                       │
│   [O]      Mostra in Esplora risorse                           │
│   [C]      Copia il percorso                                   │
│   [D]      Sposta nel Cestino                                  │
│   [E]      Elimina definitivamente                             │
│                                                                │
╰─────────────────────────────────────────────────────── 1,76 GB ╯
```

## Le tre schede

### 1 · Esplora

L'albero delle cartelle ordinato per dimensione. Ogni riga mostra dimensione, barra, quota della
cartella superiore, nome ed etichetta quando la voce è riconosciuta:

| Etichetta | Significato |
|---|---|
| `✗ sistema` | protetta: non si elimina dal programma |
| `↻ rigenerabile` | compilati, dipendenze o cache: si ricreano da soli |
| `✓ pulibile` | una voce di pulizia nota (temporanei, cestino…) |
| `! attenzione` | si può eliminare ma si perde qualcosa (modelli scaricati, dati di app…) |
| `± repository git` | un progetto con git: è al sicuro solo ciò che è committato e pushato |

Le cartelle con tanti file piccoli li mostrano raggruppati in `(N file sotto 1 MB)` per tenere
bassa la memoria.

### 2 · File grandi

I 500 file più grandi trovati dall'analisi, con la loro cartella. `T` porta al file in Esplora.

### 3 · Pulizia

```text
╭ Disk Space TUI ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ C:\  ██████░░░░░░░░░░░░░░░░░░  24,4%  liberi   2,81 TB  su 3,72 TB  · NTFS                                                               │
│ G:\  ███████░░░░░░░░░░░░░░░░░  28,2%  liberi   2,67 TB  su 3,72 TB  · FAT32                                                              │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
  1  Esplora   2  File grandi (13)   3  Pulizia (10,1 GB)
╭ Cosa si può pulire ────────────────────────────────────────────────────────────────────────╮╭ Dettagli ──────────────────────────────────╮
│   SICURO — si cancella senza conseguenze ─────────────────────────────────────── 2,13 GB   ││Cestino                                     │
│▶  [✓]    ≥29,6 MB  Cestino                                                                 ││                                            │
│   [✓]    ≥2,02 GB  File temporanei (utente)                                                ││Dimensione     29,6 MB                      │
│   [ ]        ≥0 B  File temporanei (Windows)                                               ││                                            │
│   [✓]     54,2 MB  Download di Windows Update                                              ││ ✓ SICURO                                   │
│   [✓]    ≥2,79 MB  Segnalazioni errori di Windows                                          ││File eliminati in attesa: finché non lo     │
│   [✓]     26,7 MB  Dump dei crash dei programmi                                            ││svuoti lo spazio resta occupato             │
│   CACHE — si riscaricano da sole quando servono ──────────────────────────────── 4,15 GB   ││                                            │
│   [ ]     6,49 KB  Cache pip                                                               ││Cosa fa: Svuota il Cestino (definitivo)     │
│   [ ]      447 MB  Cache Chrome                                                            ││                                            │
│   [ ]     53,6 MB  Cache Edge                                                              ││ Spazio  seleziona                          │
│   [ ]      156 MB  Cache VS Code                                                           ││ Invio  pulisci                             │
│   [ ]      441 MB  Estensioni VS Code scaricate                                            ││ N  analizza la cartella                    │
│   [ ]      257 MB  Cache cargo (crate scaricati)                                           ││ M  tutte le azioni                         │
│   [ ]     1,75 GB  Sorgenti crate estratti                                                 ││                                            │
│   [ ]     1,04 GB  Cache Gradle                                                            ││C:\$Recycle.Bin                             │
│   [ ]     37,5 MB  Repository Maven locale                                                 ││                                            │
│   FILE DI COMPILAZIONE DEI PROGETTI — si ricreano con la build ───────────────── 3,22 GB   ││                                            │
│   [ ]     1,46 GB  Projects\rust-cli › target                                              ││                                            │
│   [ ]     1,17 GB  Projects\web-shop › .next                                               ││                                            │
│   [ ]      600 MB  Projects\web-shop › node_modules                                        ││                                            │
│   CON ATTENZIONE — si perde qualcosa di scaricato ─────────────────────────────── 605 MB   ││                                            │
│   [ ]      605 MB  Modelli Hugging Face                                                    ││                                            │
│   DA GESTIRE CON LA LORO APP — qui solo per sapere quanto pesano ──────────────── 178 GB   ││                                            │
│            720 MB  Cache Google Drive                                                      ││                                            │
│           29,4 GB  Docker                                                                  ││                                            │
│           91,1 GB  Modelli Ollama                                                          ││                                            │
│           44,9 GB  Modelli LM Studio                                                       ││                                            │
│           11,5 GB  Download                                                                ││                                            │
╰─────────────────────────────────────────── ✓ 5 voci selezionate · 2,13 GB   Invio  pulisci ╯╰────────────────────────────────────────────╯
 ℹ Voci sicure selezionate: 5 · Invio = pulisci
  Spazio  seleziona   A  tutte le sicure   X  nessuna   Invio  pulisci   M  azioni   N  analizza   F5  ricalcola   ?  aiuto
```

Le voci sono raggruppate per rischio, ogni gruppo con il suo totale:

- **Sicuro** — temporanei, cestino, dump dei crash, download degli aggiornamenti.
- **Cache** — cache dei gestori di pacchetti e dei browser: si riscaricano quando servono.
- **File di compilazione dei progetti** — trovati dall'analisi: `node_modules`, `target`, `.next`,
  `.turbo`, `bin`/`obj`, cartelle di build CMake… Ognuno è riconosciuto **solo se accanto c'è il
  file di progetto** (`package.json`, `Cargo.toml`, `*.csproj`, `*.sln`, `CMakeCache.txt`, `build.gradle`).
- **Con attenzione** — es. modelli AI scaricati: si possono eliminare ma si riscaricano lentamente.
- **Da gestire con la loro app** — Ollama, LM Studio, Docker, Download… mostrati solo per sapere
  quanto pesano; il pannello Dettagli indica il comando giusto (`ollama rm`, `docker system prune`…).

## Sicurezza

Eliminare file è la parte pericolosa di un programma di pulizia: le regole sono rigide e coperte da test.

**Mai eliminabili dal programma** (l'azione viene rifiutata con il motivo):

- la radice di ogni disco e il punto di montaggio di un altro disco (Linux/macOS);
- **Windows**: `Windows` (tranne `Windows\Temp` e la cache dei download di Windows Update),
  `Program Files`, `Program Files (x86)`, `ProgramData`, `Users`, `System Volume Information`,
  `$Recycle.Bin`, `Recovery`, `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`;
- **Linux/macOS**: `/bin`, `/boot`, `/dev`, `/etc`, `/lib*`, `/proc`, `/run`, `/sbin`, `/snap`,
  `/srv`, `/sys`, `/usr`, `/var` (tranne `/var/tmp`), `/nix`, `/System`, `/Library`, `/private` e
  tutto ciò che contengono; le cartelle `/home`, `/Users`, `/root`, `/opt`, `/Applications`,
  `/mnt`, `/media`, `/Volumes`, `/tmp` stesse;
- il tuo **profilo utente** e le sue cartelle principali (Desktop, Documenti, Download, Immagini,
  Musica, Video, AppData / Library / `.config` / `.local`), e ciò che le contiene;
- tutto ciò che sta in `~/.ssh` e `~/.gnupg` (chiavi private).

**Avvisi** nella conferma per repository git, vault di Obsidian, programmi installati, dati di
applicazioni e modelli AI gestiti da un'app.

**Pulizie automatiche** (scheda Pulizia e `--clean-temp`):

- non entrano mai in un **repository git** (cartella con `.git`) — un worktree finito nei
  temporanei può contenere lavoro non salvato; vengono contati e segnalati come «lasciati intatti»;
- cartelle temporanee: solo file **più vecchi di 24 ore**; su Linux/macOS saltano anche le voci
  nascoste di primo livello (`/tmp/.X11-unix`, …), le cartelle `systemd-private-*`, socket, pipe e dispositivi;
- i file in uso vengono saltati e contati, mai forzati.

**Eliminare** da Esplora o File grandi sposta **nel cestino per impostazione predefinita** (`Canc`).
L'eliminazione definitiva (`Maiusc+Canc`) ha **Annulla** come pulsante predefinito, salvo per le
cartelle di compilazione riconosciute. I link simbolici e le junction vengono rimossi come link:
**il loro bersaglio non viene mai seguito**. Spostare nel cestino non libera spazio finché non lo
svuoti: il programma lo dice e il cestino è una voce della scheda Pulizia.

## Cosa si pulisce, per sistema

Compaiono solo le voci che esistono sul tuo computer; le dimensioni si misurano in background.

| Gruppo | Windows | Linux | macOS |
|---|---|---|---|
| Sicuro | Cestino · `%TEMP%` · `Windows\Temp` · download di Windows Update · cache Ottimizzazione recapito · Segnalazione errori · dump dei crash · installer NVIDIA | Cestino (`~/.local/share/Trash`) · `/tmp` · miniature | Cestino (`~/.Trash`) · `$TMPDIR` · `~/Library/Logs` |
| Cache | npm · Yarn · pip · Chrome · Edge · VS Code | npm · Yarn · pip · Go · Chrome · Chromium · Firefox · VS Code | `~/Library/Caches` · Xcode DerivedData · iOS DeviceSupport · cache dei simulatori · npm |
| Cache (tutti) | registro cargo · NuGet · Gradle · Maven | ← uguale | ← uguale |
| Attenzione | modelli Hugging Face | ← uguale | ← uguale · archivi di Xcode |
| A mano | cache di Google Drive · Docker · Windows.old · Ollama · LM Studio · Download | Docker (`/var/lib/docker`) · Ollama · LM Studio · Download | backup di iPhone/iPad · Docker · Ollama · LM Studio · Download |

Alcune voci di Windows (`Windows\Temp`, Windows Update, WER) richiedono di **avviare il programma
come amministratore**; altrimenti vengono misurate in parte (con `≥`) e pulite per quanto i
permessi consentono.

## Riga di comando

Lo stesso motore e le stesse protezioni, senza interfaccia: utile negli script.

```sh
# Rapporto: cartelle più grandi (ad albero), file più grandi, file di compilazione, voci di pulizia
disk-space-tui --report C:\                # oppure: --report /  ·  --report ~/progetti
disk-space-tui --report ~ --top 15 --depth 3

# Elenco dei file di compilazione in TSV: <byte> <TAB> <percorso> <TAB> <descrizione>
disk-space-tui --artifacts ~/progetti > artefatti.tsv

# Elimina DEFINITIVAMENTE i percorsi elencati in un file (uno per riga); quelli protetti sono rifiutati
cut -f2 artefatti.tsv > da-eliminare.txt
disk-space-tui --delete-list da-eliminare.txt

# Svuota la cartella temporanea (file più vecchi di 24 ore, repository git intatti)
disk-space-tui --clean-temp

disk-space-tui --help · --version · --lang it|en
```

`--delete-list` stampa una riga per percorso (`OK`, `PARZIALE`, `PROTETTO`, `ASSENTE`) e il totale.
L'eliminazione è **definitiva**: controlla prima l'elenco.

## Come si misurano le dimensioni

- Le dimensioni sono quelle **logiche dei file** (come `ls -l` o la colonna «Dimensione» di
  Esplora risorse), sommate per cartella.
- I file con più **hard link** vengono contati una volta per link: lo store di pnpm, `WinSxS` e
  alcuni strumenti di backup sembrano quindi più grandi dello spazio che occupano davvero. I
  `node_modules` installati da pnpm sono segnalati nella descrizione.
- **Link simbolici e junction non vengono seguiti**, così niente è contato due volte.
- Su Linux/macOS l'analisi **resta sul file system di partenza** (come `du -x`): analizzando `/`
  vengono saltati `/proc`, `/sys` e gli altri dischi montati.
- Le cartelle non leggibili sono segnate *non leggibili* e contate nel riepilogo.

## Note per sistema e problemi frequenti

- **Windows**: meglio usare Windows Terminal. Per pulire `Windows\Temp`, Windows Update e WER avvia
  il terminale *come amministratore*. Il Cestino si svuota con `Clear-RecycleBin` di PowerShell.
- **Linux**: «Copia il percorso» richiede `wl-copy` (Wayland), `xclip` o `xsel`; «Mostra nel gestore
  file» usa `xdg-open`. Usa `sudo` solo se devi davvero analizzare cartelle di sistema.
- **macOS**: per svuotare `~/.Trash` e leggere alcune cartelle il terminale deve avere
  l'**Accesso completo al disco** (Impostazioni di Sistema → Privacy e sicurezza).
- **Il mouse non funziona**: il terminale deve supportare il mouse; la tastiera funziona sempre.
- **I bordi si vedono male**: usa un font con i caratteri di disegno (Cascadia, JetBrains Mono,
  Fira Code, Menlo…) e un terminale UTF-8.
- **L'analisi è lenta su un disco di rete**: dipende dalla rete; analizza una sottocartella.

## Compilare dai sorgenti

```sh
git clone https://github.com/danielegabrovec/disk-space-tui
cd disk-space-tui
cargo build --release          # eseguibile in target/release/
cargo test                     # test di unità e dell'interfaccia (disegnata in un terminale in memoria)
cargo clippy --all-targets -- -D warnings
```

Stampare tutte le schermate come testo, senza un terminale vero (comodo per rivedere le modifiche):

```sh
DUMP_ROOT=/una/cartella DUMP_LANG=it cargo test --release dump_screens -- --ignored --nocapture
```

## Struttura del progetto

```text
src/main.rs      avvio, mouse, opzioni da riga di comando
src/app.rs       stato, tasti, mouse, operazioni in background (thread + canale)
src/ui.rs        disegno (ratatui)
src/scan.rs      analisi parallela (rayon), albero, file grandi, riconoscimento dei file di compilazione
src/cleanup.rs   voci di pulizia per sistema, protezioni, eliminazione, cestino, appunti
src/assess.rs    valutazioni «si può eliminare?» per il pannello Dettagli e le etichette
src/report.rs    rapporto testuale
src/i18n.rs      testi italiano/inglese (macro tr!/trf!) e lingua del sistema
src/util.rs      formattazione di dimensioni e numeri
```

## Contribuire

Segnalazioni e pull request sono benvenute: vedi [CONTRIBUTING.md](CONTRIBUTING.md). Le modifiche
che toccano eliminazione o protezioni devono avere i loro test.

## Licenza

[MIT](LICENSE) © 2026 Daniele Gabrovec
