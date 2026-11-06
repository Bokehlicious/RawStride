# Testing Strategy for PhotoWorkflow

## Inledning


## 1. Golden-output Principer

Golden-output definierar referensutdata mot vilka nya build:s kan jämföras. Följande principer gäller:

### 1.1 Metadata-handtering

#### Semantisk verifiering
- Kontrollera fältstruktur och värdebetydelse (t.ex. IPTC-fältsnamn, XMP-struktur).
- Verifiera att värden har meningsfull betydelser inom kontexten.

#### Round-trip-verifiering
- Säkerställ att läst/skrivet behåller originaldata.
- Testa att metadata håller efter en fullständig pipeline (läs → bearbeta → skrive).

#### Byte-exakta jämförelser

## 2. Testnivåer och Användningsfall

Testerna organiseras i fyra nivåer:

### 2.1 Fast Gate

**Syfte**: Grundläggande funktionstester som alltid ska lyckas.
- **När används**: Alla branch-merge, release-build:s.
- **Innehåller**:
  - Grunda compilationskontroller
  - API-kompatibilitet (om tillämpligt)
  - Metadata-round-trip för kända format

### 2.2 Riktade Tester

**Syfte**: Tester för specifika funktionalitetsområden eller problemområden.
- **När används**: Lokala förändringar inom området (t.ex. RAW-dekodning, GPU-rendering).
- **Innehåller**:
  - Format-specifika tester (RAW, JPEG, TIFF)
  - Funktionstester för nya features
  - Regressionstester för ändrade områden

### 2.3 Omfattande Tester

**Syfte**: Helhetstest inkl. långtgående prestanda och memory-checks.
- **När används**: Merge till huvudgren vid högre risk/omfattning.
- **Kriterier för när omfattande tester krävs**:

## 4. CI-strategi (Dokumentation)

### 4.1 Testnivåer Beskrivning

Följande testnivåer ska implementeras när CI-workflows skapas:

| Workflow | Syfte | Krävs för |
|----------|-------|-----------|
| `fast-gate` | Grunda funktionstest, snabb feedback | Alla merge-förfrågningar |
| `targeted-tests` | Tester specifika för ändrade områden | Lokala uppdateringar, små förändringar |
| `comprehensive-tests` | Helhetstest inkl. långtgående prestanda och memory-checks | Merge till main vid motiverad risk/omfattning |
| `release-controls` | Prestanda-minne-kompatibilitet inför release | Release-brancher, paketering |

### 4.2 När Omfattande Tester Krävs

Omfattande tester (`comprehensive-tests`) ska krävas före merge när:

1. **Ändringens risk och omfattning motiverar det** (t.ex. core-kod, renderer).
2. **Nya beroenden läggs till eller förändras**.
3. **Arvet mellan byggmiljöer påverkas** (OpenMP, GPU-rendering).

### 4.3 Dokumentationsstatus

- `.github/workflows/ci.yml` ska **inte** skapas under denna uppgift.
- Teststrategin dokumenteras här; implementation sker i framtida projektstadier.

## 5. Analysfiler

Analysfiler för specifika undersökningar finns i `docs/analyses/`:

| Fil | Syfte |
|-----|-------|
| `raw-decoding-analysis.md` | Undersökning av RAW-dekodningsvariationer mellan byggmiljöer |
| `iptc-metadata-validation.md` | Detaljerad analys av IPTC-metadatabehov och round-trip-verifiering |
| `adr0008-decision-matrix.md` | Beslutsmatris för ADR0008 (t.ex. rendering pipeline-avgöranden) |

## 6. Referenser

- [`AGENTS.md`](../AGENTS.md#3-golden-output-och-teststrategi) – Gemensamma testprinciper
- [`docs/clean-room.md`](docs/clean-room.md) – Clean-room policy för legal scoping
- `docs/adr/` – Arkitekturbeslut och tekniska val

---

*Detta dokument är en dokumentation av teststrategin. Implementering görs senare när CI-workflows skapas.*
  - Ändringens risk och omfattning motiverar det (t.ex. core-kod, renderer).
  - Nya beroenden läggs till eller förändras.
  - Arvet mellan byggmiljöer påverkas (OpenMP, GPU-rendering).

### 2.4 Releasekontroller

**Syfte**: Prestanda, minne och kompatibilitetskontroller inför release.
- **När används**: Framväxande versioner, paketering.
- **Innehåller**:
  - Memory-leak-detection (valfri verktyg)
  - Prestandabenchmarking
  - Cross-platform kompatibilitet (om tillämpligt)

## 3. Jämförelsestrategier

Följande jämförelsemétoder används beroende på data och kontext:

| Strategi | Användningstidpunkt | Exempel |
|----------|---------------------|---------|
| **Byte-exakt** | Metadata, deterministic beräkningar | Hashvärden, numeriska konstanter |
| **Toleransbaserad** | GPU-rendering, preprocessor-resultat | Pixeltillstånd, shader-output |
| **Perceptuell** | Rendering jämförelser | LPIPS/DSSIM som *möjliga* verktyg |

### 3.1 Byte-exakt Jämförelse
- Metadata-hashvärden (MD5/SHA).
- Kända numeriska konstanter (π, e, etc.).
- Filformat med exakt specifikationer (t.ex. JPEG-kvalitet 100%).

### 3.2 Toleransbaserad Jämförelse
- GPU-rendering (max diff < 1 pixel).
- Preprocessor-resultat där miljöviktiga (OpenMP parallelisering).
- Timestamp-dependent värden undviks.

### 3.3 Perceptuell Jämförelse
- Rendering jämförelser (LPIPS/DSSIM som *möjliga* verktyg).
- Brukardrivna testfall (manuell granskning vid otydlig automatisk detektering).

**Viktigt**: LPIPS, SSIM och DSSIM är *möjliga* verktyg, inte obligatoriska beroenden. Tröskelvärden bestäms empiriskt genom benchmarking.
- Används där det är meningsfullt (t.ex. metadata-hashern, exakta numeriska värden).
- Undvik byte-exakt jämförelse när miljövariationer är förväntade (t.ex. timestamping, UUID:er).

### 1.2 Deterministiska CPU-operationer

- Reproducerbara tester för preprocessor-koder och binära beräkningar.
- Byte-exakta jämförelser där lämpligt och inte meningslöst (t.ex. matematiska konstanter, hashvärden).
- Förbjudet att använda randomisering eller tidpunktsberoende värden i golden-output.

### 1.3 RAW-dekodning

#### Verifierade referenser
- Kända filformat har verifierade referensutdata (t.ex. Adobe DNG-specifikationen, Apple ProRAW).
- Dekoderingsresultat jämförs mot kända referenser där tillgängligt.

#### Miljövariationer
- Undersökning av faktisk variation mellan byggmiljöer tillåts för dokumentation.
- Långtgående miljöavhängighet (t.ex. GPU-driver-version) dokumenteras i `docs/analyses/`.

### 1.4 GPU-rendering

#### Toleransbaserad jämförelse
- Pixeltillstånd kan ha skillnader mellan byggmiljöer (render-ordering, floating-point precision).
- Toleranser ska bestämmas empiriskt genom benchmarking av referensbuild:s.
- Perceptuell jämförelse förekommer (LPIPS/DSSIM som *möjliga* verktyg, inte obligatoriska).

#### Rendering-varying faktorer
- GPU-rendering tolererar vissa skillnader (t.ex. shader-execution order, floating-point round-off).
- Byte-exakta jämförelser ska undvikas när rendering är inblandad.
Detta dokument beskriver teststrategin för PhotoWorkflow-projektet. Det definierar principer för guldstandardutdata (golden-output), testnivåer och när olika typer av tester ska användas.

**Viktigt**: Testimplementering görs inte i denna uppgift. Detta är en dokumentation av strategin, som senare implementeras i CI-workflows.
