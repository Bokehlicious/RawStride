# CLAUDE.md – Integration Instruktioner

## 1. Hänvisning till gemensamma regler

- All grundpolicy finns i [`AGENTS.md`](../AGENTS.md).
- Referera aldrig tillbaka detta dokument utan att först kolla gemensamma regler.

## 2. Projektstruktur

### Huvuddokumentation
| Fil | Syfte |
|-----|-------|
| [`AGENTS.md`](../AGENTS.md) | Gemensamma projektregler |
| [`docs/clean-room.md`](docs/clean-room.md) | Clean-room policy (nivå A–E) |

## 3. Arbetsregler för Claude

- **Skapa inte** nya ADRs utan instruktion.
- **Inga commits/pushar** under denna uppgift.
- **Ändra inte** produktionskod, tester eller befintliga filer.
- **Tillåtna ändringar**: `AGENTS.md`, `CLAUDE.md`, `docs/TESTING.md`.
- **Förbjudet**: `docs/clean-room.md`, `.github/workflows/`, ADRs 0001–0008.

## 4. Git-behållande

- Inga commits eller pushar.
- Bevara båda worktrees; arbeta enbart i `PhotoWorkflow-GUI`.

## 5. Verifieringsinstruktioner

### Legal scoping (REA/Ghidra)
1. Kontrollera `docs/clean-room.md` för nivådefinitioner.
2. Nivå A–D är tillåtna; E kräver projektägares godkännande.
3. Slut inga juridiska bedömningar – hänvisa alltid till dokumentet.

### Teststrategi-frågor
1. Referera till `AGENTS.md` sektion 3.
2. Beskriv testnivåerna (fast gate, riktade, omfattande, release) utan implementation.
3. LPIPS/DSSIM trösklar ska inte infogas som krav – dokumentera som möjliga verktyg.

### CI-frågor
1. Beskriv testnivåer och användningsfall.
2. Skapa inte `.github/workflows/ci.yml` under denna uppgift.
3. Hänvisa till `AGENTS.md` sektion 4 för policy.

## 6. Arbetsflöde

### Sökinformation
1. `AGENTS.md` → Gemensamma regler
2. Relevant ADR → Tekniska beslut
3. `docs/clean-room.md` → Legal scoping
4. Specifika analysfiler → Detaljer

### Vid ändringsbekräftelse
Använd endast:
- `editor` – Skapa/uppdatera tillåtna filer
- `run_commands` – Statusverifiering, inga ändringar
- Inget commit/push-produktionskod

## 7. Referenser

### Internt
- [`AGENTS.md`](../AGENTS.md) – Gemensamma regler
- [`docs/clean-room.md`](docs/clean-room.md) – Clean-room policy
- [`docs/rea-workflow.md`](docs/rea-workflow.md) – Verifieringsinstruktioner

## 8. Quick Reference

| Fråga | Svar |
|-------|------|
| REA/Ghidra? | Se `docs/clean-room.md`. Nivå A–D tillåtet; E kräver godkännande. |
| Testnivåer? | Se `AGENTS.md` sektion 3.2. Fyra nivåer dokumenteras, ingen implementation. |
| CI-workflows? | Dokumentera strategin. Skapa inte filer under denna uppgift. |
| P6c-filer? | Huvudworktree (`C:\Development\PhotoWorkflow`), orörd. Allt arbete i `PhotoWorkflow-GUI`. |

---

*Detta dokument är en integrationshänsyftare till [`AGENTS.md`](../AGENTS.md).*

### ADRs
Beslut dokumenteras i `docs/adr/0001` till `0008`. Se mallen i `docs/adr/TEMPLATE.md`.

### Analysfiler
Specifika mätningar finns i:
- `docs/analyses/` – Detaljerade rapporter (RAW-dekodning, IPTC-metadatabehov)