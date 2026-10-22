# SHEAF – Milestone 10: Korrektur + sichere Passwort-Pipeline

> Basis: Commit `f618b3c` (Agent-Milestone 9: Watermark + Protect/Unlock).
> Dieses Dokument schließt die Lücken von M9 und baut die Passwort-Unterstützung sicher auf.
> `SHEAF_MILESTONE_9.md` bleibt als Spezifikation für Overlay-Engine, `watermark` und `number` gültig (Abschnitte 3-6).
> Abschnitt **1** als Prompt an den Agent, Abschnitt **2** an `AGENTS.md` anhängen.

---

# 0. Statusprüfung (Fakten aus `--help` und `git log`)

| Punkt | Befund | Schwere |
|---|---|---|
| `sheaf number` | **existiert nicht** (`unrecognized subcommand`) | 🔴 fehlt aus M9 |
| `protect --password`, `--owner-password` | Passwörter als **CLI-Argument** (Shell-History, `ps`) | 🔴 Sicherheit |
| Passwort-Support außerhalb `unlock` | nicht belegt, vermutlich nur `unlock` | 🔴 sonst scheitert jeder Befehl an verschlüsselten PDFs |
| `protect --no-modify` | fehlt (nur `--no-print`, `--no-copy`) | 🟡 |
| Overlay-Engine (Rotation, Offset, `q…Q`, Resources) | **nicht belegt** | 🟡 Audit nötig |
| Verschlüsselung "128-Bit AES" | okay, aber AES-256 ungeprüft | 🟡 begründen |
| Agent-Übersicht nennt `--chunk-size`, `--pages` bei `split` | stimmt nicht, `split --help` zeigt `--ranges`/`--every`/`--bookmarks-level` | 🟡 **Selbstbericht des Agents ist unzuverlässig** |
| Commits | ein linearer Verlauf pro Milestone, keine Branches/PR erkennbar | 🟡 |
| `split`, `merge`, `info`, `verify`, `capabilities` | entsprechen dem Plan | 🟢 |

**Konsequenz:** Dokumentation und `capabilities` müssen **aus dem Code generiert** werden, nicht vom Agent von Hand geschrieben (10A.4).

---

# 1. Prompt an den Agent

```text
Milestone 9 ist teilweise abgenommen. Starte Milestone 10: Korrektur + sichere Passwort-Pipeline.
Lies zuerst SHEAF_MILESTONE_10.md und SHEAF_MILESTONE_9.md (Abschnitte 3-6) komplett.

Arbeite auf dem Branch `feat/m10-hardening`. Pushe den Branch, merge NICHT nach main (das mache ich per PR).

Teil A (Korrektur), je Schritt ein Commit:
10A.1  Overlay-Audit: Tabelle aus Abschnitt 3 mit Beleg (Testname) oder Fix ausfüllen
10A.2  `sheaf number` auf der Overlay-Engine bauen (Spec: SHEAF_MILESTONE_9.md §5)
10A.3  Desktop: Overlay-Panel um "Seitenzahlen" und Live-Vorschau ergänzen, falls noch nicht vorhanden
10A.4  CLI-Doku und `capabilities` aus clap generieren + Docs-Sync-Test

Teil B (Passwörter), je Schritt ein Commit:
10B.1  Argv-Passwörter entfernen (hidden Flags mit Migrationsfehler)
10B.2  Passwort-Quellen: env / file / stdin, SecretString
10B.3  Zentrale Lade-Pipeline `load_pdf`, alle lesenden Befehle nutzen sie (clippy erzwingt das)
10B.4  `protect` / `unlock` überarbeiten, `info` zeigt Verschlüsselung
10B.5  Desktop: Passwort-Dialog, Protect-Panel, kein Disk-Cache für verschlüsselte PDFs
10B.6  Tests laut Abschnitt 9, Secret-Leak-Test ist Pflicht

Regeln:
- Keine Crates oder APIs raten, docs.rs prüfen, Versionen nennen.
- Prüfe in lopdf 0.46.0 (Quellcode), welche Verschlüsselung erzeugt und gelesen werden kann (RC4, AES-128, AES-256).
  Falls AES-256 fehlt: dokumentiere das im Bericht, verwende AES-128, biete KEIN RC4 an.
- Keine Features außerhalb dieses Dokuments. Ideen am Ende als Vorschlag.
- Dein Bericht muss auf Fakten basieren: füge die Ausgabe von `sheaf capabilities --json`,
  `sheaf <cmd> --help` für protect/unlock/number und `cargo test`-Zusammenfassung bei.

Am Ende: Zusammenfassung (gebaut, Abweichungen, offen) und Vorschlag für Milestone 11 (Metadaten).
Starte Milestone 11 NICHT ohne meine Freigabe.
```

---

# 2. Ergänzung für `AGENTS.md`

````markdown
## Secrets (verschärft)
- Passwörter nie als CLI-Argument. Nur `--*-password-env`, `--*-password-file`, `--*-password-stdin`.
- Passwörter nur als `secrecy::SecretString`. Kein `Debug`/`Display`, kein Logging, nicht in JSON, Fehlern oder Snapshots.
- Höchstens EIN `--*-stdin` pro Aufruf.
- Entschlüsselte Daten werden nie in Temp-Dateien außerhalb des Zielordners geschrieben und nie im Disk-Cache abgelegt.

## PDF laden
- Jede Operation lädt PDFs ausschließlich über `sheaf_core::load::load_pdf`.
- `lopdf::Document::load*` direkt aufrufen ist per `clippy.toml` (`disallowed-methods`) verboten.

## Dokumentation
- `docs/CLI.md` und `sheaf capabilities` werden aus der clap-Definition generiert, nicht von Hand gepflegt.
- Der Docs-Sync-Test schlägt fehl, wenn `docs/CLI.md` vom generierten Stand abweicht.
- Abschlussberichte enthalten rohe Tool-Ausgaben, keine Paraphrasen der CLI.
````

---

# 3. Teil A – Korrektur

## 10A.1 Overlay-Audit

Für jede Zeile: **Beleg** (Testname) oder **Fix** im Commit. Die Tabelle gehört in den Abschlussbericht.

| # | Frage | Erwartung |
|---|---|---|
| 1 | Läuft `watermark` über eine gemeinsame `ops::overlay`-Engine? | ja, sonst refactoren |
| 2 | Wird Originalinhalt in `q … Q` gekapselt? | ja |
| 3 | Geerbte/indirekte `/Resources` werden ins Seiten-Dict kopiert (Parent unverändert)? | ja |
| 4 | `/Contents` als Referenz, Array und leer funktionieren? | ja, je ein Test |
| 5 | `/Rotate` 90/180/270 visuell korrekt (Render + Pixelprüfung)? | ja |
| 6 | `MediaBox`-Offset und `CropBox` berücksichtigt? | ja |
| 7 | Resource-Namen `Sheaf*` kollisionsgeprüft? | ja |
| 8 | Nicht-WinAnsi-Text → `unsupported` mit Hint, kein Panic? | ja |
| 9 | Deckkraft über `ExtGState` (`/ca`, `/CA`)? | ja |
| 10 | Text bleibt selektierbar (Extraktion im Test)? | ja |

Spezifikation und Matrizen: `SHEAF_MILESTONE_9.md` §3.

## 10A.2 `sheaf number`

Vollständig nach `SHEAF_MILESTONE_9.md` §5 (Platzhalter `{n}`, `{total}`, `{page}`, `{pages}`, `--start`, `--pages`, Positionen, Default `{stem}_numbered.pdf`). Nutzt dieselbe Overlay-Engine wie `watermark`.

## 10A.3 Desktop-Overlay-Panel

- Panel "Overlay" mit Segmented Control **Wasserzeichen | Seitenzahlen**.
- Live-Vorschau der ersten ausgewählten Seite (Debounce 300 ms, veraltete Anfragen verwerfen).
- Falls das Watermark-Panel schon existiert: erweitern statt neu bauen.

## 10A.4 Doku und `capabilities` aus Code generieren

| Artefakt | Quelle | Prüfung |
|---|---|---|
| `docs/CLI.md` | `sheaf docs` (hidden Subcommand, nutzt `clap::CommandFactory`, z. B. via `clap-markdown` oder eigener Renderer) | Test vergleicht Datei mit generierter Ausgabe, Update per `UPDATE_DOCS=1 cargo test` |
| `sheaf capabilities` | clap-Introspektion (`Command::get_subcommands`, `get_arguments`) statt handgepflegter Liste | Snapshot-Test (`insta`) |
| README | verlinkt `docs/CLI.md`, enthält nur Beispiele | Test: jeder Befehl aus `capabilities` hat mindestens ein Beispiel in der README |

`capabilities`-Eintrag je Befehl: `name`, `args[]` mit `long`, `value_name`, `required`, `secret` (bei Passwort-Quellen), `status`.

---

# 4. Teil B – Passwort-Pipeline

## 10B.1 Argv-Passwörter entfernen

| Alt | Neu |
|---|---|
| `protect --password`, `--owner-password` | entfällt |
| `unlock --password` | entfällt |
| Übersicht des Agents: `--user-password` | entfällt |

- Alte Flags bleiben als `hide = true` erhalten und brechen mit **Exit 2** ab:
  `"--password wurde entfernt (Sicherheitsrisiko). Nutze --password-env VAR, --password-file PFAD oder --password-stdin."`
- Kein Deprecation-Zeitraum nötig (noch kein Release). Schema-Version bleibt `1`.
- Fehlercode für falsche Credential-Quellen: den bestehenden Usage-Fehlercode verwenden (wie clap-Parsefehler). Falls keiner existiert: `usage` einführen, Exit 2.

## 10B.2 Passwort-Quellen

| Gruppe | Befehle | Flags (gegenseitig exklusiv je Gruppe) |
|---|---|---|
| **Open** (Passwort der Eingabedatei) | alle lesenden Befehle | `--password-env <VAR>`, `--password-file <PFAD>`, `--password-stdin` |
| **New user** | `protect` (Pflicht) | `--new-password-env`, `--new-password-file`, `--new-password-stdin` |
| **New owner** | `protect` (optional) | `--new-owner-password-env`, `--new-owner-password-file` |

Regeln:
- **Höchstens ein** `*-stdin` pro Aufruf, sonst Usage-Fehler.
- `*-stdin` liest **eine Zeile**, entfernt nur das abschließende `\n`/`\r\n` (keine anderen Whitespaces). Ist stdin ein TTY: Usage-Fehler mit Hint `printf '%s' "$PW" | sheaf ...` (nie interaktiv fragen).
- `*-env`: Variable fehlt oder leer → Usage-Fehler.
- `*-file`: genau ein abschließender Zeilenumbruch wird entfernt. Ist die Datei für Gruppe/Andere lesbar (`mode & 0o077 != 0`): **Warning**, kein Fehler.
- Leeres Passwort → Usage-Fehler.
- Typ `secrecy::SecretString`, `zeroize` (best effort, lopdf kopiert intern).
- Mehrere Eingaben (`merge`): dasselbe Open-Passwort wird bei **jeder** verschlüsselten Eingabe probiert, unverschlüsselte ignorieren es.

## 10B.3 Zentrale Lade-Pipeline

```rust
// crates/sheaf-core/src/load.rs
pub struct LoadOptions { pub password: Option<SecretString> }

pub struct LoadedPdf { pub doc: lopdf::Document, pub was_encrypted: bool }

/// Einziger erlaubter Einstieg zum Laden von PDFs.
pub fn load_pdf(path: &Path, opts: &LoadOptions) -> Result<LoadedPdf, SheafError>;
```

| Situation | Ergebnis |
|---|---|
| nicht verschlüsselt | lädt normal, Passwort wird ignoriert |
| verschlüsselt, kein Passwort | `Encrypted` (Exit 4) |
| verschlüsselt, falsches Passwort | `WrongPassword` (Exit 4) |
| verschlüsselt, richtiges Passwort | Dokument entschlüsselt im Speicher |

- **Erzwingen:** `clippy.toml` mit `disallowed-methods` für `lopdf::Document::load`, `load_from`, `load_mem` und ähnliche (Namen in der Doku prüfen). Ausnahme nur in `load.rs`.
- Alle `*Options`-Structs bekommen `input: LoadOptions` (oder die Op nimmt `&LoadOptions` als Parameter).
- **PDFium (render, to-images, Thumbnails):** Passwort an `pdfium-render` beim Öffnen übergeben.
- Betroffene Befehle: `merge`, `split`, `pages`, `render`, `compress`, `convert to-images`, `convert to-markdown`, `watermark`, `number`, `protect`, `unlock`, `info`, `verify`.

## 10B.4 Ausgabe-Regeln

| Regel | Detail |
|---|---|
| Ausgabe ist **unverschlüsselt**, wenn die Eingabe verschlüsselt war (außer `protect`) | Warning im Envelope: `"Eingabe war verschlüsselt, Ausgabe ist nicht verschlüsselt."` |
| `info` ohne Passwort bei verschlüsselter Datei | Exit 0, `encrypted: true`, `pages: null`, `encryption`-Block (aus unverschlüsseltem `/Encrypt`-Dict lesbar) |
| `info` mit Passwort | vollständige Infos |
| `verify` ohne Passwort bei verschlüsselter Datei | Exit 4 `encrypted` (Inhalt nicht prüfbar) |
| `unlock` bei unverschlüsselter Eingabe | Exit 0, Warning `"Datei ist nicht verschlüsselt"`, Ausgabe = Kopie |
| `protect` bei verschlüsselter Eingabe | braucht Open-Passwort (sonst Exit 4), verschlüsselt neu |
| Passwort in Envelope, Logs, `--verbose`, Fehlern | **nie** |

## `info`: neuer Block `encryption` (additiv, Schema bleibt `1`)

```json
"encryption": {
  "algorithm": "aes-128",
  "key_bits": 128,
  "revision": 4,
  "permissions": { "print": false, "copy": true, "modify": true }
}
```

- `null`, wenn nicht verschlüsselt.
- Permissions aus `/P` (signed 32-bit): Bit 3 = Drucken, Bit 4 = Ändern, Bit 5 = Kopieren.

## 10B.5 `protect` / `unlock`

```bash
sheaf protect report.pdf --new-password-env SHEAF_PW -o locked.pdf [--new-owner-password-env OWNER_PW] [--no-print] [--no-copy] [--no-modify]
printf '%s' "$PW" | sheaf unlock locked.pdf --password-stdin -o open.pdf
SHEAF_PW=secret sheaf info locked.pdf --password-env SHEAF_PW --json
```

| Aspekt | Entscheidung |
|---|---|
| Algorithmus | AES-256 (V5/R6), falls `lopdf` es kann, sonst AES-128 (V4/R4). **Kein RC4**, auch nicht optional |
| Owner-Passwort | ohne Angabe = User-Passwort |
| Berechtigungen | `--no-print`, `--no-copy`, `--no-modify` (neu) |
| Hinweis in Doku und `--help` | Berechtigungen sind **Hinweise an den Viewer**, keine echte Sperre. Echter Schutz = User-Passwort |
| Default-Ausgabe | `{stem}_protected.pdf`, `unlock`: `{stem}_unlocked.pdf` |
| Report `protect` | `{input, output, algorithm, key_bits, permissions}` (kein Passwort) |

## 10B.6 Desktop

| Element | Verhalten |
|---|---|
| **Passwort-Dialog** | erscheint beim Öffnen/Droppen eines verschlüsselten PDFs. Feld `type=password`, `Enter` bestätigt, `Esc` bricht ab, falsches Passwort → Inline-Fehler im Dialog, kein Toast |
| **Passwort-Haltung** | nur im Speicher (Rust-State, `SecretString`), wird beim Schließen der Datei verworfen, nie persistiert, nie im Frontend-Store geloggt |
| **Protect-Panel** (`⌘7`) | Passwort + Bestätigung, "Anzeigen"-Toggle, unter "Erweitert": Owner-Passwort, Berechtigungs-Toggles. Warnhinweis bei < 8 Zeichen (kein Zwang) |
| **Thumbnail-Cache** | für verschlüsselte PDFs **nur im RAM**, nie auf Disk (sonst liegt entschlüsselter Inhalt unverschlüsselt im Cache-Ordner). Cache-Key enthält kein Passwort |
| **Verschlüsselungs-Badge** | in der Statusleiste: Schloss-Icon + "AES-128/256" |
| Befehle | `open_pdf` bekommt optionales Passwort, `protect`, `unlock` als Tauri-Commands. `types.ts` aktualisieren, `npm run check` und `npm run build` grün |

---

# 5. Fehler-Mapping (unverändert, zur Kontrolle)

| Situation | `code` | Exit |
|---|---|---|
| verschlüsselt, kein Passwort | `encrypted` | 4 |
| falsches Passwort | `wrong_password` | 4 |
| ungültige Credential-Quelle (env fehlt, stdin-TTY, zwei stdin, leeres Passwort, entfernte Flags) | Usage-Code | 2 |
| Ausgabedatei existiert | `output_exists` | 7 |

---

# 9. Tests

| Ebene | Werkzeug | Inhalt |
|---|---|---|
| **Secret-Leak (Pflicht)** | `assert_cmd` | Passwort `LEAK-CANARY-7f3a` über env/file/stdin an `protect`, `unlock`, `info`, `split`, `watermark` übergeben, jeweils mit `--json`, `--verbose`, `--progress json`, Fehlerfall und `--dry-run`. **Weder stdout noch stderr enthält den Canary** |
| Roundtrip | Core | `protect → unlock`: Seitenzahl und extrahierter Text identisch |
| Fremdvalidierung | `qpdf` | `qpdf --check --password=…` und `qpdf --show-encryption` bestätigen Algorithmus und Berechtigungen |
| PDFium | `pdfium-render` | verschlüsselte Ausgabe lässt sich mit Passwort rendern, ohne Passwort nicht |
| Quellen | `assert_cmd` | env fehlt/leer, file fehlt, stdin ist TTY, zwei stdin-Flags, leeres Passwort → Exit 2 |
| Entfernte Flags | `assert_cmd` | `--password`, `--owner-password`, `--user-password` → Exit 2 mit Migrationshinweis |
| Pipeline | Core | jeder lesende Befehl: ohne Passwort → 4 `encrypted`, falsch → 4 `wrong_password`, richtig → Erfolg + Warning "Ausgabe ist nicht verschlüsselt" |
| Lint-Erzwingung | CI | `cargo clippy` schlägt bei direktem `Document::load` außerhalb `load.rs` fehl |
| `info` | Snapshot | `encryption`-Block für AES, Permissions-Bits korrekt dekodiert |
| Permissions | Core | `--no-print/--no-copy/--no-modify` setzen genau die erwarteten `/P`-Bits |
| Datei-Rechte | Unix-Test | `--password-file` mit Mode `0644` → Warning |
| Cache | Core/Desktop | Render eines verschlüsselten PDFs legt **keine** Datei im Thumbnail-Cache an |
| `number` | wie `SHEAF_MILESTONE_9.md` §7 | Rotation, Offset, Platzhalter, Rendering |
| Docs-Sync | CI | `docs/CLI.md` entspricht generierter Ausgabe, `capabilities`-Snapshot |
| UI | `npm run check` | TypeScript fehlerfrei |

---

# 10. Akzeptanzkriterien

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` grün
- [ ] Overlay-Audit-Tabelle vollständig, Fixes mit Tests belegt
- [ ] `sheaf number` existiert, nutzt dieselbe Overlay-Engine wie `watermark`, steht in `capabilities`
- [ ] **Kein** Befehl akzeptiert ein Passwort als Argument, alte Flags brechen mit Exit 2 und Migrationshinweis
- [ ] Secret-Leak-Test grün (Canary nirgends in stdout/stderr)
- [ ] Alle lesenden Befehle laden über `load_pdf`, clippy erzwingt das
- [ ] Verschlüsselte Eingaben: Exit 4 ohne/mit falschem Passwort, Erfolg mit richtigem, Ausgabe unverschlüsselt mit Warning
- [ ] `protect` verwendet AES (kein RC4), `qpdf --show-encryption` bestätigt es, Berechtigungs-Bits stimmen
- [ ] `info` zeigt `encryption`-Block, ohne Passwort Exit 0
- [ ] Desktop: Passwort-Dialog, Protect-Panel, **kein Disk-Cache** für verschlüsselte PDFs
- [ ] `docs/CLI.md` und `capabilities` aus clap generiert, Docs-Sync-Test grün
- [ ] Bericht enthält rohe Tool-Ausgaben (`capabilities`, `--help`, Testzusammenfassung)
- [ ] Arbeit auf `feat/m10-hardening`, Branch gepusht, nicht in `main` gemerged

---

# 11. Roadmap danach

| # | Milestone | Inhalt |
|---|---|---|
| 11 | Metadaten | `meta get\|set\|strip` (nutzt `load_pdf`) |
| 12 | Release | Universal Binary, PDFium x86_64 + arm64 bundlen, Signing, Notarization, `.dmg`, Homebrew Cask, `completions`, Stresstests (500+ Seiten) |
| 13 | OCR | durchsuchbare PDFs (macOS Vision per FFI oder `ocrs`/Tesseract) |
| 14 | Redact | echtes Schwärzen, gründliche Tests |
| später | LaTeX/Typst | nur als optionaler externer Wrapper (`from-latex` via `tectonic`/`latexmk` im PATH), kein PDF → LaTeX |
