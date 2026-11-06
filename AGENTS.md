# AGENTS.md – Gemensamma Projektregler för PhotoWorkflow

## Inledning

Detta dokument innehåller gemensamma regler, principer och konventioner som gäller hela PhotoWorkflow-projektet. Dessa regler gäller oavsett vilken del av kodgrupperingsstrukturen eller arbetsmiljö de tillämpas i.

## 1. Worktree-separation och miljöisolering

### 1.1 Arbetsytor
Projektet använder två separata Git-worktrees med tydliga ansvar:

| Worktree | Syfte | Branch | Status |
|----------|-------|--------|--------|
| `C:\Development\PhotoWorkflow` | Huvudworktree med prototypkod (P6c-arbete) | `m2-and-adr0008-phase1` | Orörlig – alla ocommittade förändringar bevaras orörda |
| `C:\Development\PhotoWorkflow-GUI` | Arbetsyta och utvecklingsmiljö | `feature/gui-workspace`, `m2-and-adr0008-phase1` | Aktiv utveckling |

### 1.2 Regler för worktree-användning
- **Inga förändringar i huvudworktree**: Filer i `C:\Development\PhotoWorkflow` ska inte ändras, röjas eller flyttas under denna uppgift.
- **Arbetsytan är ensam tillämpningsområde**: Alla dokumentuppdateringar, kodförändringar och experiment sker enbart i `C:\Development\PhotoWorkflow-GUI`.
- **Separata worktrees för isolering**: Worktree-separation används för att upprätthålla gränser mellan prototyputveckling och produktionsarbete. Detta är ett arbetsregelbaserat avgränsande, inte ett tekniskt skrivskydd.

### 1.3 Filsortering mellan worktrees

## 2. Laglig Scoping och Clean-room Policy

### 2.1 Metodnivåer för studier (enligt `docs/clean-room.md`)
Följande nivåer definieras för studier av Photo Mechanic:

| Nivå | Metodik | Status |
|------|---------|--------|
| **A** | Normal användning, beteendestudier, prestandamätningar | Tillåtet (alla) |
| **B** | Passivt OS-nivåövervakning (Task Manager, Process Monitor, WPR/ETW) | Tillåtet med ägarens godkännande |
| **C** | Studier av utdata och filformat (XMP, IPTC, filnamn) | Tillåtet |
| **D** | Offentligt material (dokumentation, licenser, ADRs) | Tillåtet |
| **E** | Analys av binärer (REA, Ghidra, debuggare, dekompilering) | Förbjudet – kräver projektägares separata juridiska bedömning |

### 2.2 Nivå E som separat fråga
- Nivå E betraktas inte som automatiskt förbjuden utan istället som en separat juridisk och projektägarstyrd fråga.
- Dekompilering, REA-analys eller användning av Ghidra kräver projektagares explicita godkännande per fall.
- `docs/clean-room.md` ska inte ändras under denna uppgift; dess policy för nivå A–D förblir oförändrad.

### 2.3 Tillåtna verktyg och material
- **REA, Ghidra**: Enbart för projektets egna byggnader (proprietär nivå E kräver godkännande).
- **Open-source beroenden** (t.ex. LibRaw): Fri användning enligt kompatibla licenser.
- **Övrigt proprietärt material**: Får inte placeras i repot.

## 3. Golden-output och Teststrategi

### 3.1 Principer för guldstandardutdata
Följande principer gäller för test- och validationsstrategin:

#### Metadata-handtering
- **Semantisk verifiering**: Kontrollera fältstruktur och värdebetydelse.
- **Round-trip-verifiering**: Säkerställ att läst/skrivet behåller originaldata.
- **Byte-exakta jämförelser**: Används där det är meningsfullt (t.ex. metadata-hasher, exakta värden).

#### Deterministiska CPU-operationer
- Reproducerbara tester för preprocessor-koder och binära beräkningar.
- Byte-exakta jämförelser där lämpligt och inte meningslöst (t.ex. matematiska konstanter).

#### RAW-dekodning
- Verifierade referenser för kända filformat.
- Undersökning av faktisk variation mellan byggmiljöer tillåts för dokumentation.

#### GPU-rendering
- Toleransbaserad jämförelse där det behövs (rendering, pixeltillstånd).
- Pixeltillstånd kan ha skillnader mellan byggmiljöer; toleranser ska bestämmas empiriskt.

### 3.2 Testnivåer och deras användning
Testerna organiseras i fyra nivåer:

| Nivå | Beskrivning | När används |
|------|-------------|-------------|
| **Fast gate** | Grundläggande funktionstester som alltid ska lyckas. | Alla branch-merge, release. |
| **Riktade tester** | Tester för specifika funktionalitetsområden eller problemområden. | Lokala förändringar inom området. |
| **Omfattande tester** | Helhetsfunktionstest som krävs när risken/komplexiteten motiverar det. | Merge till huvudgren vid högre risk/omfattning. |
| **Releasekontroller** | Prestanda, minne och kompatibilitetskontroller inför release. | Framväxande versioner, paketering. |

### 3.3 Jämförelsestrategier
- **Byte-exakt**: Metadata, deterministic beräkningar.
- **Toleransbaserad**: GPU-rendering, preprocessor-resultat där miljöviktiga.
- **Perceptuell**: Rendering jämförelser (LPIPS/DSSIM som *möjliga* verktyg, inte obligatoriska).

### 3.4 Cache Policy – separat utredning
- Cachepolicy är definierad som en separat teknisk utredning.
- Ingen fast minnesbudget, FIFO, LRU eller annan eviction-policy ska införas i detta dokument.
- Benchmarkanpassning och implementationsdetaljer är inte del av denna uppgift.

## 4. CI-strategi (dokumentation endast)

### 4.1 Testnivåer beskrivning
Följande testnivåer ska implementeras när CI-workflows skapas:

## 5. Agent-instruktioner och verktygsspecifika regler

### 5.1 Dokumentation uppdelning
- **`AGENTS.md`**: Gemensamma projektregler (detta fil).
- **`CLAUDE.md`**: Hänsyftar till gemensamma regler i `AGENTS.md`, duplicerar inte hela dokumentet.
- **Cline-specifika instruktioner**: Hålls separat från gemensamma agentregler.

### 5.2 EditorConfig och liknande integrationsdetaljer
- `.editorconfig`-regler ska inte presenteras som om de var Claude Code-integration; dessa är standardverktygskedjeregler.
- Dokumentationen bör fokusera på projektets egna arbetsflöden.

## 6. Kodkvalitet och arkitektur

### 6.1 ADR-struktur (Architecture Decision Records)
- Varje teknisk avgörelse dokumenteras som en egen ADR i `docs/adr/`.
- ADRs följer mallen i `docs/adr/TEMPLATE.md`.
- Beslutsstatus: `accepted`, `proposed`, eller `open` med datumsstämpel.

### 6.2 Ingen ärvda kod eller assets
- Proprietärt material från konkurrenter (t.ex. Photo Mechanic) får inte placeras i repot.
- All observation och analys av andra produkter görs utanför huvudrepot.

## 7. Tilläggsresurser

### 7.1 Analysfiler
Analysfiler för specifika undersökningar finns i:
- `docs/analyses/` – Detaljerade mätningar och jämförelser (t.ex. RAW-dekodning, IPTC-metadatabehov).

### 7.2 Prototypmapparna
Utvecklingsprototyper är organiserade i:
- `prototypes/phase*/` – Stadiebaserad prototyputveckling för GPU-rendering och liknande områden.

## Referenser
- [docs/clean-room.md](docs/clean-room.md) – Detaljerad clean-room policy.
- [docs/rea-workflow.md](docs/rea-workflow.md) – Verifieringsinstruktioner för tillåtna metoder.
- `docs/adr/` – Arkitekturbeslut och tekniska val.

| Workflow | Syfte | Krävs för |
|----------|-------|-----------|
| `fast-gate` | Grunda funktionstest, snabb feedback | Alla merge-förfrågningar |
| `targeted-tests` | Tester specifika för ändrade områden | Lokala uppdateringar, små förändringar |
| `comprehensive-tests` | Helhetstest inkl. långtgående prestanda och memory-checks | Merge till main vid motiverad risk/omfattning |
| `release-controls` | Prestanda-minne-kompatibilitet inför release | Release-brancher, paketering |

### 4.2 När omfattande tester krävs
Omfattande tester (`comprehensive-tests`) ska krävas före merge när:
- Ändringens risk och omfattning motiverar det (t.ex. core-kod, renderer).
- Nya beroenden läggs till eller förändras.
- Arvet mellan byggmiljöer påverkas (OpenMP, GPU-rendering).

### 4.3 Dokumentationsstatus
- `.github/workflows/ci.yml` ska inte skapas under denna uppgift.
- Teststrategin dokumenteras här; implementation sker i framtida projektstadier.

- Proprietary material från externa produkter (t.ex. Photo Mechanic) förvaras utanför repot.
- Observations och analyser av konkurrenters produkter görs i separata projektutrymmen.
- Endast neutral, självformulerad kravställning tas in i `docs/`.
