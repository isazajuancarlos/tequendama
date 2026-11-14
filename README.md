<!--
SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
SPDX-License-Identifier: MIT
-->
# tequendama

**Ingestión de datos externos, sin reinventarla en cada importador.**

Barrido del universo (2026-09-22): 18 importadores en 7 repos reinventaban las
mismas cinco cosas, cada uno distinto. Esto las reúne:

- **`http`** — cliente `reqwest` (blocking) con timeout, reintentos y ritmo integrados.
- **`reintentos`** — `con_reintentos(politica, f)`: backoff exponencial con techo
  (el diseño de polymarket, el único bien hecho del barrido).
- **`ritmo`** — limitador de intervalo mínimo entre llamadas.
- **`Capturado<T>`** — la envoltura de PROCEDENCIA: `{ fuente, id_externo,
  capturado_en, dato }`. Todo lo que entra queda auditable.
- **`ledger`** — registro de cobertura REANUDABLE: qué ventanas/claves ya se
  trajeron, para que un import por lotes retome donde quedó.
- **`csv`** — se adopta el crate `csv` (se mató el `split(',')` naíf).

- **`socrata`** — cliente `datos.gov.co`: builder SoQL (`$select/$where/$group/
  $order/$limit/$offset`), ejecución **paginada** por offset, y extracción de
  campos (Socrata manda los números como string). Extraído del patrón que los
  tres conectores hermanos —IDEAM, riego, calidad— copiaban.

No sube lo idiosincrático (semántica de cada fuente —departamentos, patrones LIKE,
ventaneo por trimestre, índices—): eso se queda en su repo.

El piloto (bloque 1.3) es colapsar los tres conectores de `red-hidrica-enso` sobre
este crate.
