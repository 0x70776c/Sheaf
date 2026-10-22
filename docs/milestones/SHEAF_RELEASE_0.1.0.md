# SHEAF – Release 0.1.0 (Open Source, ohne Apple-Account)

> Ersetzt Teil B von `SHEAF_MILESTONE_12.md` (Signing/Notarization entfällt) und zieht nur das Release-Kritische aus Teil A vor.
> Abschnitt **1** als Prompt an den Agent, Abschnitt **2** an `AGENTS.md` anhängen.
> Das Icon liegt bereits fertig bei (Ordner `icon/`), siehe Abschnitt 3.

---

# 0. Entscheidungen

| Thema | Entscheidung |
|---|---|
| Distribution | Open Source auf GitHub, Artefakte über GitHub Releases |
| Apple Developer Account | **keiner**. Kein Signing mit Developer ID, keine Notarization |
| Code-Signatur | **Ad-hoc** (`codesign -s -`). Auf Apple Silicon ist mindestens das Pflicht, sonst startet die Binary nicht |
| Gatekeeper | Browser-Downloads tragen Quarantäne, die App wird beim ersten Start blockiert. Das wird in der README ehrlich erklärt (Abschnitt 8) |
| Vertrauensersatz | `SHA256SUMS`, GitHub-Build-Provenance-Attestation, reproduzierbarer Build aus dem Quellcode, Bau-Anleitung |
| Homebrew | nur Vorlagen im Repo (eigener Tap), kein Eintrag in `homebrew/cask` (Richtlinien für nicht notarisierte Casks vorher prüfen) |
| Architekturen | Universal (arm64 + x86_64). Fällt x86_64 aus (z. B. PDFium), Fallback arm64-only mit Hinweis im Bericht |
| Auto-Updater | keiner (100 % offline) |

**Aus M12 vorgezogen (release-kritisch):** `--version`, PDFium-Suchpfad, Completions, Panic-Hook, Tauri-Härtung, Lizenz-Check, Versions-Sync.
**Nach dem Release (Backlog):** Korrupt-Input-Matrix, `cargo-fuzz`, Benchmarks (`criterion`), Offline-Beweis per `sandbox-exec`, Formulare/Signaturen, OCR (M13), Redact (M14).

---

# 1. Prompt an den Agent

```text
Wir veröffentlichen Sheaf 0.1.0 als Open-Source-Projekt auf GitHub. Es gibt KEINEN Apple-Developer-Account:
kein Signing mit Developer ID, keine Notarization. Lies docs/milestones/SHEAF_RELEASE_0.1.0.md komplett
(committe die Datei dorthin, falls sie fehlt; ich lege sie bereit, ebenso den Ordner icon/).

Reihenfolge (je Schritt ein Commit, Arbeitsbranch `chore/release-0.1.0`, siehe Abschnitt 4 für Git-Regeln):

R1  Git-Bestandsaufnahme und Sicherung (nur lesen + Backup-Bundle)
R2  Branches mergen, Repo bereinigen (.gitignore, getrackte Artefakte, Branches)
R3  Icon integrieren
R4  Release-kritischer Code (--version, PDFium-Suchpfad, Completions, Panic-Hook, Tauri-Härtung, Lizenzen)
R5  Build- und Packaging-Skripte (Universal, Ad-hoc-Signatur, DMG, Tarball, Checksums)
R6  GitHub-Actions: CI und Release-Workflow
R7  Dokumentation und Community-Dateien
R8  Verifikation, lokaler Tag, Abschlussbericht

Harte Regeln:
- Kein force-push, kein History-Rewrite, kein `git branch -D`, keine Löschung von Branches, die nicht nachweislich in main gemergt sind.
- Vor jeder Änderung an Branches: Backup mit `git bundle create ../sheaf-backup-<datum>.bundle --all`.
- Gefundene Secrets oder große Dateien in der Historie: STOPPEN und melden, nicht selbst bereinigen.
- Keine Apple-Zugangsdaten, keine Secrets in Dateien, Logs oder Berichten.
- Tag `v0.1.0` nur LOKAL anlegen, NICHT pushen. Gib mir die Befehle zum Pushen.
- Keine Crates, Tools oder Befehle raten: Doku prüfen, Versionen nennen.
- Keine Features außerhalb dieses Dokuments.
- BERICHTSFORMAT wie in M11: Spec-Ist-Tabelle mit Belegen und rohe Tool-Ausgaben (Abschnitt 9).

Am Ende: Zusammenfassung, offene Punkte, Befehle für meinen Teil (Tag pushen, Release veröffentlichen).
```

---

# 2. Ergänzung für `AGENTS.md`

````markdown
## Release (Open Source, ohne Apple-Account)
- Releases entstehen nur aus einem Git-Tag `vX.Y.Z` über `release.yml`.
- Keine Signing-Secrets. Artefakte sind ad-hoc signiert und werden mit `SHA256SUMS` und Build-Provenance ausgeliefert.
- Reproduzierbar: `rust-toolchain.toml`, `Cargo.lock`, `package-lock.json` committet, Builds mit `--locked` / `npm ci`.
- Versionsnummer an einer Stelle (Workspace-`Cargo.toml`), `scripts/bump-version.sh` synchronisiert Tauri und npm, ein Test erzwingt Gleichheit.
- GitHub Actions: Berechtigungen minimal (`contents: read`, Schreibrechte nur im Release-Job), Actions auf feste Versionen pinnen (Commit-SHA bevorzugt).

## Git
- Kein force-push auf `main`, kein History-Rewrite ohne ausdrückliche Freigabe.
- Merge von Milestone-Branches mit `--no-ff`, Branches danach löschen (lokal und remote), sobald nachweislich gemergt.
- Nur Quellen committen: keine Build-Artefakte, Binaries (`*.dylib`), `node_modules`, `target`, generierte Fixtures.

## Fehlerkontrakt bei Abstürzen
- Ein Panic erzeugt bei `--json` trotzdem genau ein JSON-Envelope (`internal`, Exit 1), nie Secrets.
````

---

# 3. R3: Icon

## Konzept

| Idee | Umsetzung |
|---|---|
| **Sheaf = Garbe** | fünf Halme, unten gebündelt, oben gefächert |
| **Halme = Seiten** | schlichte, gleichförmige Streifen, keine PDF-Klischees (kein Dokument-Symbol, keine Ecke) |
| **Das Band ist ein Schnitt** | Das Binden entsteht durch **Negativraum** (Lücke im Bündel) mit einem goldenen Band: oben getrennt (Split), unten zusammengehalten (Merge) |
| **Mathematische Garbe** | lokale Teile, die sich konsistent zu einem Ganzen fügen |
| **Material** | Graphit-Kachel, Elfenbein-Halme, ein einziger Goldakzent (Getreide) |

Kleine Größen: Silhouette und Goldband bleiben bis 32 px klar erkennbar, bei 16 px bleibt die Garbenform lesbar (siehe `icon-preview-sizes.png`).

## Mitgelieferte Dateien (nach `assets/icon/` ins Repo legen)

| Datei | Zweck |
|---|---|
| `icon-1024.png` | **verbindliche Quelle**, 1024 × 1024 RGBA, Kachel 824 px mit Apple-Rand und Schatten bereits eingebaut |
| `icon.svg` | Vektorquelle. **Im Entwicklungsumfeld nicht visuell verifiziert** (kein SVG-Renderer vorhanden), Agent prüft sie (unten) |
| `make_icon.py` | erzeugt das PNG reproduzierbar (Pillow + NumPy), alle Parameter oben im Skript |
| `make_svg.py` | erzeugt die SVG aus denselben Parametern |
| `icon-preview-sizes.png` | Kontrollbild (256/128/64/32/16 px auf hell und dunkel) |

## Parameter (für den Fall, dass die Dateien fehlen oder angepasst werden)

| Element | Wert |
|---|---|
| Kachel | Superellipse n = 5, Halbkante 412 (824 px), Verlauf `#2E2C29` → `#141312`, zarter Glanz oben, 1-px-Lichtkante oben |
| Halme | 5 Stück, Breite 50, Winkel −36°/−18°/0°/18°/36° um einen gemeinsamen Drehpunkt, Längen 412/438/450/438/412, Verlauf `#FBF7EE` → `#E6DDCA` |
| Fuß | Halme laufen 126 px unter den Drehpunkt, flach abgeschnitten |
| Band | Lücke 80 px hoch am Drehpunkt (Negativraum), darin Goldbalken 240 × 44, Radius 22, Verlauf `#F2C55C` → `#DEA232` |
| Schatten | Kachel: Blur 14, Versatz 12, 42 %. Mark: Blur 10, Versatz 14 |

## Integration (Schritte)

1. Dateien nach `assets/icon/` kopieren und committen.
2. **SVG prüfen:** `qlmanage -t -s 1024 -o /tmp assets/icon/icon.svg` (oder `rsvg-convert`), Ergebnis mit `icon-1024.png` vergleichen. Weicht es sichtbar ab, SVG verwerfen oder korrigieren (PNG gilt), im Bericht vermerken.
3. Icon-Set erzeugen: `cargo tauri icon assets/icon/icon-1024.png` im Ordner `apps/desktop` (Tauri-CLI-Version nennen). **Kein zusätzliches Padding und keine eigene Rundung**, beides steckt schon im PNG.
4. Aufräumen: nur behalten, was `tauri.conf.json` braucht (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.png`). Windows-/iOS-/Android-Reste löschen, `bundle.icon` anpassen.
5. Prüfen: `iconutil -c iconset apps/desktop/src-tauri/icons/icon.icns -o /tmp/sheaf.iconset`, Größen 16 bis 512@2x vorhanden, zwei Größen ansehen.
6. Einbauen: `docs/images/icon-256.png` (für README, per `sips -Z 256`), Über-Dialog der App, DMG-Volume-Icon.
7. `tauri dev`: Dock-Icon sichtbar (nur wenn lokal prüfbar, sonst im Bericht vermerken).

Optional später: Icon-Composer-Varianten (dunkel/getönt) für macOS 26. Nicht Teil von 0.1.0.

---

# 4. R1/R2: Git bereinigen und mergen

## R1: Bestandsaufnahme (nur lesen, plus Sicherung)

```bash
git status --short
git fetch --all --prune
git bundle create ../sheaf-backup-$(date +%F).bundle --all
git branch -a -vv
git log --oneline --graph --all | head -60
git branch --merged main
git branch --no-merged main
git ls-files | wc -l
git ls-files -z | xargs -0 du -k | sort -nr | head -20          # größte getrackte Dateien
git count-objects -vH
git log --format='%an <%ae>' | sort | uniq -c                   # Autor-Identitäten
git grep -nE '/Users/|/home/[a-z]+' -- . ':!*.lock' | head      # lokale Pfade in Dateien
```

Ergebnis als Tabelle im Bericht: Branches (gemergt/nicht gemergt), größte Dateien, getrackte Artefakte, Autor-Identitäten, lokale Pfade.

**Secrets-Scan:** `gitleaks detect` falls installiert, sonst `git log -p --all | grep -nE 'BEGIN (RSA|EC|OPENSSH) PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|password\s*='` auf Treffer prüfen. **Treffer in der Historie: stoppen und melden.**

## R2: Merges und Bereinigung

| Schritt | Regel |
|---|---|
| Reihenfolge | `feat/m10-hardening` → `feat/m11-meta` (zuerst prüfen: `git merge-base --is-ancestor feat/m10-hardening feat/m11-meta`; ist m10 in m11 enthalten, genügt der Merge von m11) |
| Methode | `git checkout main && git merge --no-ff <branch> -m "Merge milestone N: <titel>"` |
| Vor jedem Merge | auf dem Merge-Ergebnis: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm run check`, `npm run build` |
| Konflikte | sauber lösen, im Bericht auflisten. Unklare Konflikte: stoppen und fragen |
| Push | normaler `git push origin main` (kein `--force`) |
| Branches löschen | nur Branches, die `git branch --merged main` listet: `git branch -d`, dann `git push origin --delete <branch>` |
| Unbekannte/ungemergte Branches | **nicht löschen**, auflisten und fragen |
| Remote aufräumen | `git remote prune origin` |

Arbeitsbranch für den Rest: `chore/release-0.1.0` von `main`, am Ende mit `--no-ff` zurück nach `main` mergen und pushen, Branch löschen.

## `.gitignore` (Mindestinhalt, an Repo anpassen)

```gitignore
# Rust
/target/
**/*.rs.bk

# Node / Tauri
node_modules/
apps/desktop/dist/
apps/desktop/src-tauri/target/
apps/desktop/src-tauri/gen/

# PDFium und Build-Ausgaben
/vendor/pdfium/
/dist/
*.dylib
*.dmg
*.app/
*.tar.gz

# Tests
*.snap.new
tests/fixtures/generated/

# macOS / Editor
.DS_Store
.idea/
.vscode/
*.swp

# Lokal/Secrets
.env
.env.*
*.p12
*.pem
*.key
```

- `Cargo.lock` und `package-lock.json` **bleiben getrackt** (Binary-Projekt, reproduzierbare Builds).
- Bereits getrackte Artefakte (`target`, `vendor/pdfium`, `*.dylib`, `.DS_Store`, `*.snap.new`, `node_modules`) mit `git rm -r --cached` entfernen, in einem eigenen Commit `chore: untrack build artifacts`.
- Fixtures: programmatisch erzeugte große PDFs nicht committen (`tests/fixtures/generated/`), kleine statische Fixtures (< 200 KB) dürfen bleiben. Entscheidung und Größen im Bericht.
- `.gitattributes`: `*.pdf binary`, `*.png binary`, `*.icns binary`, `*.snap text eol=lf`.
- Lokale Pfade (`/Users/...`) in getrackten Dateien (Doku, Snapshots) entfernen oder neutralisieren.
- **Autor-Identität:** Nur berichten, **nicht umschreiben**. Künftige Commits nutzen die GitHub-noreply-Adresse (Empfehlung im Bericht, Entscheidung beim Besitzer).

---

# 5. R4: Release-kritischer Code

| Punkt | Vorgabe |
|---|---|
| **Version** | `sheaf --version` → `sheaf 0.1.0 (commit abc1234, 2026-10-22)`, per `build.rs` aus Git, ohne Netzwerk (`unknown` bei fehlendem Git). `capabilities` additiv: `version`, `commit`, `tools.pdfium {found, path}` |
| **Versions-Sync** | Workspace-`Cargo.toml` = `tauri.conf.json` = `package.json`, Test + `scripts/bump-version.sh` |
| **PDFium-Suche** | zentral in `sheaf-core`, Reihenfolge: 1) `PDFIUM_LIB_PATH`, 2) neben der Executable, 3) `../lib/`, 4) `../Frameworks/` (App-Bundle), 5) Systempfade. Nichts gefunden → `unsupported` (Exit 8) mit Liste der probierten Orte. Befehle ohne Rendering laufen weiter. Kein Download zur Laufzeit |
| **Completions** | `sheaf completions <bash|zsh|fish|elvish|powershell>` (`clap_complete`, API prüfen), Skript auf stdout, mit `--json` im Envelope. In `capabilities`, `docs/CLI.md`, README. Test je Shell |
| **Panic-Hook** | früh in `main`, `--json` vor dem clap-Parsing aus `argv` erkennen. Bei `--json`: genau ein Envelope `{ok:false, error:{code:"internal", exit_code:1, ...}}`, Exit 1. Panic-Ort nur mit `--verbose` auf stderr. Test mit `SHEAF_TEST_PANIC=1` (nur Debug/Test-Build) |
| **Tauri-Härtung** | CSP ohne Remote-Quellen (`default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'`), Berechtigungen minimal (Dialog, Events, eigene Commands, kein `shell`/`http`/`updater`/breites `fs`), jede Berechtigung im Bericht begründen. "Im Finder zeigen" über festen Rust-Command ohne Shell-Interpolation |
| **Lizenzen** | `LICENSE-MIT` und `LICENSE-APACHE` im Root (README behauptet Dual-Lizenz), `deny.toml` mit Lizenz-Allowlist (MIT, Apache-2.0, BSD-*, ISC, Unicode-*, Zlib …) und Bans für Netzwerk-Crates in **direkten** Abhängigkeiten, `cargo deny check licenses bans` grün (transitive Treffer begründen), `THIRD_PARTY_LICENSES.md` erzeugen (z. B. `cargo about`, npm-Lizenzen, PDFium-Hinweise), im DMG/Tarball enthalten |
| **Smoke auf 500 Seiten** | `info`, `merge`, `split --every 50`, `render` (20 Thumbnails) auf der vorhandenen 500-Seiten-Fixture, Zeiten und Peak-RSS (`/usr/bin/time -l`) im Bericht (keine Budgets, nur Messwerte) |

---

# 6. R5: Build und Packaging (ohne Signing-Account)

## `scripts/build-universal.sh`

| Schritt | Detail |
|---|---|
| Toolchain | `rustup target add aarch64-apple-darwin x86_64-apple-darwin`, `--locked`, `MACOSX_DEPLOYMENT_TARGET` setzen (Vorschlag 12.0, Mindestanforderungen von Tauri/WebKit und PDFium prüfen, in README nennen) |
| PDFium | `pdfium-mac-arm64.tgz` und `pdfium-mac-x64.tgz` aus `bblanchon/pdfium-binaries`, **feste Release-Version und SHA-256 im Skript**, Download nur im Build-Skript. Per `lipo -create` zu Universal-`libpdfium.dylib` |
| CLI | beide Targets bauen, `lipo -create` → `dist/sheaf` |
| Desktop | `tauri build --target universal-apple-darwin` (Doku prüfen), PDFium nach `Contents/Frameworks/` (Tauri-Option `bundle.macOS.frameworks` bzw. Ressourcen, Doku prüfen) |
| Mindest-macOS | `bundle.macOS.minimumSystemVersion` setzen |
| Signatur | **nach** allen `lipo`-/Kopierschritten, von innen nach außen: `libpdfium.dylib`, CLI, App. `codesign --force --sign - --timestamp=none` (Ad-hoc). Kein `--deep` zum Signieren |
| DMG | Tauri-DMG verwenden, sonst `hdiutil create` (Inhalt: `Sheaf.app` + Link auf `/Applications`), Volume-Icon aus `icon.icns` |
| Tarball | `sheaf-<version>-macos-universal.tar.gz`: `bin/sheaf`, `lib/libpdfium.dylib`, `completions/`, `LICENSE-*`, `THIRD_PARTY_LICENSES.md`, `README` |
| Checksums | `dist/SHA256SUMS` über DMG und Tarball |
| Ausgabe | `dist/Sheaf-<version>-universal.dmg`, `dist/sheaf-<version>-macos-universal.tar.gz`, `dist/SHA256SUMS` |

Hinweise:
- `pdfium-render` bindet PDFium per Laufzeit-Laden mit **explizitem Pfad** aus der Suchreihenfolge (Abschnitt 5). Daher ist kein Link-Zeit-`rpath` nötig.
- Lipo/Kopieren **invalidiert Signaturen**: immer erst danach signieren.
- Fallback: Scheitert x86_64, arm64-only bauen, Abschnitt "Systemvoraussetzungen" und Dateinamen anpassen (`-arm64`), im Bericht begründen.

## Prüfungen im Skript (Fehlschlag = Abbruch)

| Prüfung | Befehl | Erwartung |
|---|---|---|
| Architekturen | `lipo -archs` auf CLI, App-Binary, `libpdfium.dylib` | `x86_64 arm64` |
| Abhängigkeiten | `otool -L` auf CLI und App-Binary | nur System-Bibliotheken, keine Pfade nach `/Users`, `/opt/homebrew`, `/usr/local` |
| Signatur | `codesign --verify --deep --strict --verbose=2` auf App, CLI, Dylib | ok |
| Rosetta | `arch -x86_64 dist/sheaf capabilities --json` | läuft, `tools.pdfium.found` je nach Pfad |
| CLI mit PDFium | Tarball entpacken, `bin/sheaf render … --width 160` | rendert (PDFium über `../lib` gefunden) |
| DMG | `hdiutil attach`, enthält `Sheaf.app` und Applications-Link, `detach` | ok |
| Mindest-macOS | `defaults read Sheaf.app/Contents/Info LSMinimumSystemVersion` | gesetzter Wert |

---

# 7. R6: GitHub Actions

**`ci.yml`** (Push/PR auf `main`): macOS-Runner, Toolchain aus `rust-toolchain.toml`, Cache, `cargo fmt --check`, `cargo clippy ... -D warnings`, `cargo test --workspace`, `docs`-Sync-Test, `npm ci`, `npm run check`, `npm run build`, `cargo deny check licenses bans`. `permissions: contents: read`.

**`release.yml`**

| Aspekt | Vorgabe |
|---|---|
| Trigger | `push` von Tags `v*` und `workflow_dispatch` (Trockenlauf: baut und lädt Artefakte hoch, erstellt **kein** Release) |
| Runner | `macos-latest` (arm64), Cross-Build für x86_64 |
| Schritte | Checkout, Toolchain, Node (`npm ci`), `scripts/build-universal.sh`, Prüfungen aus Abschnitt 6, Artefakte hochladen |
| Provenance | Build-Attestation für DMG, Tarball und `SHA256SUMS` mit `actions/attest-build-provenance` (Version prüfen, benötigt `id-token: write` und `attestations: write`) |
| Release | `gh release create vX.Y.Z --draft --prerelease --notes-file <Auszug aus CHANGELOG.md>`, Artefakte anhängen. **Draft**, damit der Besitzer vor Veröffentlichung prüft |
| Rechte | Top-Level `contents: read`, nur der Release-Job `contents: write` |
| Secrets | keine außer `GITHUB_TOKEN` |
| Sicherheit | Actions auf Commit-SHA pinnen (Versionskommentar daneben), keine Interpolation von Tag-/Branch-Namen in Shell-Skripte ohne Quoting |

---

# 8. R7: Dokumentation und Community-Dateien (alles auf Englisch)

| Datei | Inhalt |
|---|---|
| `README.md` | Kopf mit `docs/images/icon-256.png`, Einzeiler, Feature-Überblick, Screenshots (falls vorhanden), **Install**, CLI-Quickstart, Privacy ("100 % offline, no telemetry"), Systemvoraussetzungen, Build from source, Lizenz |
| Install-Abschnitt | DMG-Download, Tarball für CLI, Homebrew (Hinweis auf eigenen Tap, sobald vorhanden), Quellbau |
| **Gatekeeper-Abschnitt** | Ehrlich: "Sheaf is not notarized (no paid Apple Developer account). macOS shows a warning on first launch." Dann: (1) Systemeinstellungen → Datenschutz & Sicherheit → nach dem blockierten Start **"Trotzdem öffnen"** (ab macOS 15 der vorgesehene Weg), (2) Alternative per Terminal `xattr -dr com.apple.quarantine /Applications/Sheaf.app`. Hinweis, dass der CLI-Tarball per `curl` keine Quarantäne bekommt |
| Download prüfen | `shasum -a 256 -c SHA256SUMS`, `gh attestation verify <datei> --repo <owner>/Sheaf`, Verweis auf Quellbau als stärkste Prüfung |
| `CONTRIBUTING.md` | Setup, Tests, Commit-Stil (Conventional Commits), CLI-Vertrag und `AGENTS.md`, wie man Issues/PRs stellt |
| `SECURITY.md` | Meldung über GitHub Private Vulnerability Reporting, Scope (Parser, Verschlüsselung, Metadaten-Bereinigung), Hinweis: PDF-Berechtigungen sind nur Viewer-Hinweise |
| `CHANGELOG.md` | Keep-a-Changelog, Eintrag `0.1.0` aus der Meilenstein-Historie (Features nach Befehlen gruppiert) |
| `.github/ISSUE_TEMPLATE/bug_report.md` | verlangt `sheaf --version`, `sheaf capabilities --json` (gekürzt), macOS-Version, Beispiel-Befehl (keine privaten PDFs anhängen) |
| `packaging/homebrew/` | `sheaf.rb` (Formula für CLI-Tarball), `Casks/sheaf.rb` (DMG, `postflight` entfernt Quarantäne per `xattr`, nur für den eigenen Tap, aktuelle Homebrew-Richtlinien zu nicht notarisierten Casks prüfen und im Bericht nennen). URL/SHA als Platzhalter, `scripts/update-homebrew.sh` füllt sie aus `SHA256SUMS` |
| `docs/CLI.md` | aktuell (Docs-Sync-Test), enthält `completions` |

Konsistenz: Der Produktname, die Binary (`sheaf`) und der App-Name (`Sheaf.app`) stimmen überall überein. Bundle-Identifier: den aktuell gesetzten Wert nennen. Ist es ein Platzhalter (`com.example.*`), im Bericht als TODO markieren.

---

# 9. R8: Verifikation, Tag, Bericht

## Muss-Ausgaben im Bericht

| Nr. | Inhalt |
|---|---|
| 1 | Git-Tabelle aus R1 **vorher** und `git branch -a`, `git log --oneline --graph -25` **nachher** |
| 2 | `git ls-files | wc -l`, größte getrackte Dateien, `git status --short` (leer) |
| 3 | `cargo fmt --check`, clippy, `cargo test --workspace` (echte Testzahl via `cargo test --workspace -- --list 2>/dev/null \| grep -c ': test$'`), `npm run check`, `npm run build` |
| 4 | `sheaf --version`, `sheaf capabilities --json` (gekürzt auf `version`, `commit`, `tools`) |
| 5 | Ausgaben der Prüfungen aus Abschnitt 6 (`lipo -archs`, `otool -L`, `codesign --verify`, Rosetta-Lauf, DMG-Inhalt) |
| 6 | `cargo deny check licenses bans` |
| 7 | Smoke-Messwerte auf 500 Seiten (Zeit, Peak-RSS, Maschine) |
| 8 | Icon-Prüfung: Ergebnis des SVG-Vergleichs, `iconutil`-Größenliste |
| 9 | `scripts/build-universal.sh` Auszug (letzte 30 Zeilen) und `dist/SHA256SUMS` |
| 10 | Ergebnis des Trockenlaufs von `release.yml` (`workflow_dispatch`), falls ausführbar, sonst `actionlint`-Ausgabe oder begründeter Verzicht |
| 11 | Spec-Ist-Tabelle zu allen Punkten R1 bis R8, Abweichungen ausdrücklich |

## Lokaler Tag

```bash
git tag -a v0.1.0 -m "Sheaf 0.1.0"        # nur lokal anlegen
# NICHT pushen. Befehle für den Besitzer im Bericht:
#   git push origin main
#   git push origin v0.1.0
```

---

# 10. Akzeptanzkriterien

- [ ] Backup-Bundle erstellt, kein force-push, keine History-Änderung
- [ ] Milestone-Branches mit `--no-ff` in `main` gemergt, Tests nach jedem Merge grün, gemergte Branches lokal und remote gelöscht, ungemergte Branches unangetastet und gelistet
- [ ] `.gitignore` und `.gitattributes` vorhanden, getrackte Artefakte entfernt, keine lokalen Pfade mehr, `git status` sauber
- [ ] Secrets-Scan ohne Treffer (oder gemeldet und gestoppt)
- [ ] Icon: `assets/icon/` im Repo, `tauri icon` ausgeführt, nur benötigte Icon-Dateien behalten, `.icns` enthält alle Größen, SVG verifiziert oder verworfen, Icon in README, Über-Dialog und DMG
- [ ] `--version`, `capabilities.tools.pdfium`, PDFium-Suchpfad, Completions, Panic-Hook umgesetzt und getestet
- [ ] Tauri: CSP gesetzt, Berechtigungen minimal und begründet
- [ ] `cargo deny check licenses bans` grün, `LICENSE-*` und `THIRD_PARTY_LICENSES.md` vorhanden
- [ ] Versions-Sync-Test grün (Workspace = Tauri = npm)
- [ ] `build-universal.sh` erzeugt DMG, Tarball und `SHA256SUMS`, alle Prüfungen aus Abschnitt 6 bestanden (oder arm64-Fallback begründet)
- [ ] Ad-hoc-Signatur besteht `codesign --verify --deep --strict`
- [ ] `ci.yml` und `release.yml` vorhanden, Rechte minimal, Actions gepinnt, Release als Draft/Prerelease
- [ ] README mit Install, Gatekeeper-Erklärung, Download-Prüfung, Build from source; `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, Bug-Template, Homebrew-Vorlagen
- [ ] Lokaler Tag `v0.1.0` angelegt, **nicht gepusht**
- [ ] Bericht im vorgeschriebenen Format (Spec-Ist-Tabelle plus rohe Ausgaben)

---

# 11. Dein Teil

| # | Aufgabe | Wann |
|---|---|---|
| 1 | Ordner `icon/` nach `assets/icon/` im Repo und diese Datei nach `docs/milestones/` kopieren, bevor der Agent startet | vorher |
| 2 | **Viewer-Test (offen seit M10):** Mit `sheaf protect` erzeugte PDF in Preview und im Browser öffnen. Scheitert Preview an AES-256, vor dem Release dem Agent melden (Fallback AES-128 oder `--algorithm`) | vor dem Tag |
| 3 | Autor-Identität entscheiden (Klarname/E-Mail öffentlich oder GitHub-noreply) | nach R1-Bericht |
| 4 | GitHub-Repo-Einstellungen: Beschreibung, Topics, **Private Vulnerability Reporting** aktivieren, Actions → "Workflow permissions" prüfen, nach dem Merge Branch-Schutz für `main` (CI erforderlich) | vor dem Tag |
| 5 | Befehle aus dem Bericht ausführen: `git push origin main`, `git push origin v0.1.0` | nach Abnahme |
| 6 | Draft-Release auf GitHub prüfen, Artefakte und `SHA256SUMS` ansehen, dann veröffentlichen | nach Workflow-Lauf |
| 7 | **Download-Test:** DMG aus dem Release **im Browser** laden (Quarantäne), Gatekeeper-Warnung und die Anleitung der README durchspielen, möglichst auf einem zweiten Mac oder frischen Benutzerkonto | nach Veröffentlichung |
| 8 | Optional: Homebrew-Tap `homebrew-sheaf` anlegen und Vorlagen hineinkopieren | später |
| 9 | Produktname final prüfen (GitHub, crates.io, Homebrew, DPMA/EUIPO), bei Änderung vor dem Tag, danach aufwendig | vor dem Tag |

---

# 12. Backlog nach dem Release

| # | Thema |
|---|---|
| 13 | OCR (macOS Vision per FFI oder `ocrs`/Tesseract), durchsuchbare PDFs |
| 14 | Redact (echtes Schwärzen) |
| 15 | Formulare ausfüllen, Signatur-Bild platzieren |
| Härtung | Korrupt-Input-Matrix (Truncate, Byte-Flips), `cargo-fuzz`, Benchmarks mit `criterion`, Offline-Beweis per `sandbox-exec`, Notarization falls später ein Account vorhanden ist |
| Später | LaTeX/Typst nur als optionaler externer Wrapper, kein PDF → LaTeX |
