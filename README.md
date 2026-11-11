<!--
SPDX-License-Identifier: MIT
SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
-->

# tequendama

External data ingestion for Rust — HTTP client with timeout/retry/pacing, resumable ledger, provenance wrapper, and a Socrata connector. MIT. Rust · reqwest · zero reinvention.

**English** · [Español](#español)

---

## What it does

A survey of production importers found that 18 of them, spread across several projects, were each reinventing the same five things independently and differently: HTTP fetching, retry logic, rate limiting, provenance tagging, and resumable batch tracking. tequendama gathers those five into one audited crate.

### Modules

| Module | What it provides |
|---|---|
| **`http`** | A `reqwest` blocking client pre-configured with timeout, retry policy, and minimum inter-call interval. |
| **`reintentos`** | `con_reintentos(policy, f)` — exponential backoff with a ceiling, extracted from the one importer in the survey that had it right. |
| **`ritmo`** | Minimum-interval rate limiter between consecutive calls. |
| **`Capturado<T>`** | Provenance wrapper: `{ source, external_id, captured_at, data }`. Every record that enters the system carries its origin, timestamp, and external identifier — auditable by construction. |
| **`ledger`** | Resumable coverage ledger: records which time windows or keys have already been fetched, so a batch import can restart from where it stopped without re-fetching what it already has. |
| **`csv`** | Thin wrapper over the `csv` crate that replaces the naïve `split(',')` pattern found in every importer in the survey. |
| **`socrata`** | Client for Socrata-based open-data portals: SoQL query builder (`$select`, `$where`, `$group`, `$order`, `$limit`, `$offset`), offset-based pagination, and numeric-field extraction (Socrata encodes numbers as strings). |

### What it does not do

tequendama does not carry source-specific semantics — department hierarchies, LIKE patterns, quarterly windowing, database indexes. Those belong in the importer that knows the domain.

---

## The code is not hosted here

Since August 2026 the source of this project **is not hosted on GitHub**, and it is not distributed through crates.io.

This is not abandonment, not an incident, and there was no breach. It is a decision, and it is worth saying what it rests on. Through 2026 the package registries and code-hosting platforms were hit by sustained, chained supply-chain attacks — the Shai-Hulud worm, ChainDrop in August, malicious extensions that exfiltrated thousands of private repositories. For a data-ingestion library integrated into production pipelines that handle provenance-sensitive records, hosting the source there stopped being a neutral decision.

**The licence has not changed and the code is still available to anyone who asks** — delivered with a signature and a checksum, at no cost, with no NDA, no restriction on what you publish about it, and no expiry.

To request it: **[xiliux.com](https://xiliux.com)**

---

## Español

## Qué hace

Una inspección de importadores en producción encontró que 18 de ellos, repartidos entre varios proyectos, reinventaban de forma independiente y distinta las mismas cinco cosas: consulta HTTP, lógica de reintentos, limitación de ritmo, etiquetado de procedencia y seguimiento reanudable de lotes. tequendama reúne esas cinco en un único crate auditado.

### Módulos

| Módulo | Qué aporta |
|---|---|
| **`http`** | Cliente `reqwest` blocking preconfigurado con timeout, política de reintentos e intervalo mínimo entre llamadas. |
| **`reintentos`** | `con_reintentos(politica, f)` — backoff exponencial con techo, extraído del único importador del barrido que lo tenía bien. |
| **`ritmo`** | Limitador de intervalo mínimo entre llamadas consecutivas. |
| **`Capturado<T>`** | Envoltura de procedencia: `{ fuente, id_externo, capturado_en, dato }`. Cada registro que entra al sistema lleva su origen, marca de tiempo e identificador externo — auditable por construcción. |
| **`ledger`** | Registro de cobertura reanudable: anota qué ventanas temporales o claves ya se han traído, para que un import por lotes retome donde quedó sin volver a pedir lo que ya tiene. |
| **`csv`** | Envoltorio delgado sobre el crate `csv` que sustituye el patrón naíf `split(',')` que aparecía en cada importador del barrido. |
| **`socrata`** | Cliente para portales de datos abiertos basados en Socrata: constructor SoQL (`$select`, `$where`, `$group`, `$order`, `$limit`, `$offset`), paginación por offset y extracción de campos numéricos (Socrata codifica los números como cadenas). |

### Lo que no hace

tequendama no lleva semántica específica de cada fuente — jerarquías departamentales, patrones LIKE, ventaneo por trimestre, índices de base de datos. Eso le corresponde al importador que conoce el dominio.

### El código no se publica aquí

Desde agosto de 2026, el código de este proyecto **no se aloja en GitHub**, ni se distribuye por crates.io.

No es abandono ni un incidente: es una decisión, y conviene decir por qué. A lo largo de 2026 los registros de paquetes y las plataformas de código han sufrido ataques encadenados de cadena de suministro —Shai-Hulud, ChainDrop en agosto, extensiones maliciosas que se llevaron miles de repositorios privados—. Alojar aquí el código de una librería de ingestión de datos integrada en pipelines de producción que manejan registros con trazabilidad de procedencia dejó de ser una decisión neutra.

**La licencia no cambia y el código se sigue entregando a quien lo pida**, con su firma y su suma de verificación, sin coste, sin NDA, sin restricción sobre lo que publique y sin caducidad.

Para solicitarlo: **[xiliux.com](https://xiliux.com)**

---

*tequendama* — © 2026 Juan Carlos Isaza. Licencia **MIT**.
La licencia no cambia: lo que cambia es por dónde se entrega el código.
