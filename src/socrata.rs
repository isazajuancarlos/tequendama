// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
// SPDX-License-Identifier: MIT

//! Cliente Socrata (datos.gov.co) con consultas SoQL y paginación.
//!
//! Del barrido: **tres** conectores hermanos —IDEAM, riego (ADR/UPRA), calidad
//! (IRCA)— pegan todos a `datos.gov.co/resource/<id>.json` con el mismo baile:
//! armar una consulta SoQL, llamar con timeout, y leer una lista de objetos donde
//! **todos los campos vienen como string** (hasta los números). Aquí, una vez.
//!
//! Lo que NO sube (directiva 9, «nunca el alfa»): la lista de departamentos, los
//! patrones `LIKE`, el ventaneo por trimestre y el índice del proyecto de agua.
//! Eso es dominio, y se queda en `red-hidrica-enso`. Aquí está el andamio: URL,
//! builder de SoQL, ejecución paginada y extracción de campos.

use std::fmt;

use serde_json::Value;

use crate::http::{Cliente as ClienteHttp, ErrorHttp};

/// El host por defecto de datos abiertos de Colombia.
pub const DATOS_GOV_CO: &str = "https://www.datos.gov.co";

/// Cliente contra UN dataset Socrata.
pub struct Socrata {
    /// URL del recurso, `https://<host>/resource/<dataset>.json`.
    recurso: String,
    /// Token de aplicación (opcional; sube el límite de tasa de Socrata).
    app_token: Option<String>,
    http: ClienteHttp,
}

/// Una consulta SoQL: cláusulas `$select`, `$where`, `$group`, `$order`, `$limit`,
/// `$offset`. Se arma con el patrón builder; el orden de las llamadas no importa.
#[derive(Debug, Clone, Default)]
pub struct Consulta {
    select: Option<String>,
    where_: Option<String>,
    group: Option<String>,
    order: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
}

impl Consulta {
    /// Una consulta vacía (trae el dataset completo, paginado).
    pub fn nueva() -> Self {
        Self::default()
    }
    /// `$select` — columnas o agregados (`sum(x) AS s, count(*) AS n`).
    pub fn select(mut self, s: impl Into<String>) -> Self {
        self.select = Some(s.into());
        self
    }
    /// `$where` — filtro. OJO: interpola valores; el llamante escapa lo que venga
    /// de fuera (Socrata no tiene binds).
    pub fn filtro(mut self, w: impl Into<String>) -> Self {
        self.where_ = Some(w.into());
        self
    }
    /// `$group` — agrupación (para agregados).
    pub fn agrupar(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }
    /// `$order` — orden. Necesario para que la paginación por offset sea estable.
    pub fn orden(mut self, o: impl Into<String>) -> Self {
        self.order = Some(o.into());
        self
    }
    /// `$limit` — tamaño de página.
    pub fn limite(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }
    /// `$offset` — desde qué fila (lo mueve la paginación; rara vez a mano).
    pub fn desde(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    /// Los pares `(clave, valor)` de esta consulta, para la query string.
    fn pares(&self) -> Vec<(&'static str, String)> {
        let mut p = Vec::new();
        if let Some(s) = &self.select {
            p.push(("$select", s.clone()));
        }
        if let Some(w) = &self.where_ {
            p.push(("$where", w.clone()));
        }
        if let Some(g) = &self.group {
            p.push(("$group", g.clone()));
        }
        if let Some(o) = &self.order {
            p.push(("$order", o.clone()));
        }
        if let Some(l) = &self.limit {
            p.push(("$limit", l.to_string()));
        }
        if let Some(o) = &self.offset {
            p.push(("$offset", o.to_string()));
        }
        p
    }
}

/// Lo que puede salir mal contra Socrata.
#[derive(Debug)]
pub enum ErrorSocrata {
    /// Fallo HTTP (transporte, estado, cuerpo).
    Http(ErrorHttp),
    /// La URL del dataset no se pudo construir.
    Url(String),
    /// La respuesta no fue una lista de objetos JSON como Socrata promete.
    Forma(String),
}

impl fmt::Display for ErrorSocrata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSocrata::Http(e) => write!(f, "http: {e}"),
            ErrorSocrata::Url(e) => write!(f, "url: {e}"),
            ErrorSocrata::Forma(e) => write!(f, "forma: {e}"),
        }
    }
}

impl std::error::Error for ErrorSocrata {}

impl From<ErrorHttp> for ErrorSocrata {
    fn from(e: ErrorHttp) -> Self {
        ErrorSocrata::Http(e)
    }
}

impl Socrata {
    /// Cliente para `dataset` en el host por defecto (datos.gov.co).
    pub fn nuevo(dataset: &str, http: ClienteHttp) -> Self {
        Self::en(DATOS_GOV_CO, dataset, http)
    }

    /// Cliente para `dataset` en un host Socrata cualquiera.
    pub fn en(host: &str, dataset: &str, http: ClienteHttp) -> Self {
        let host = host.trim_end_matches('/');
        Self {
            recurso: format!("{host}/resource/{dataset}.json"),
            app_token: None,
            http,
        }
    }

    /// Añade un token de aplicación (sube el límite de tasa de Socrata).
    pub fn con_token(mut self, token: impl Into<String>) -> Self {
        self.app_token = Some(token.into());
        self
    }

    /// La URL del recurso, para inspección/pruebas.
    pub fn recurso(&self) -> &str {
        &self.recurso
    }

    /// Construye la URL final de una consulta (recurso + query SoQL + token).
    fn url(&self, c: &Consulta) -> Result<reqwest::Url, ErrorSocrata> {
        let mut pares = c.pares();
        if let Some(t) = &self.app_token {
            pares.push(("$$app_token", t.clone()));
        }
        reqwest::Url::parse_with_params(&self.recurso, &pares)
            .map_err(|e| ErrorSocrata::Url(e.to_string()))
    }

    /// Ejecuta UNA consulta y devuelve las filas (una página). El http subyacente
    /// ya trae timeout, ritmo y reintentos.
    pub fn ejecutar(&self, c: &Consulta) -> Result<Vec<Value>, ErrorSocrata> {
        let url = self.url(c)?;
        let v: Value = self.http.obtener_json(url.as_str())?;
        match v {
            Value::Array(filas) => Ok(filas),
            otra => Err(ErrorSocrata::Forma(format!(
                "se esperaba una lista, vino {}",
                tipo(&otra)
            ))),
        }
    }

    /// Ejecuta una consulta PAGINADA: pide de a `tam_pagina` filas subiendo el
    /// `$offset` hasta que una página venga incompleta (o vacía). Devuelve TODAS
    /// las filas. Requiere que la consulta tenga un `$order` estable, o Socrata
    /// puede repetir/saltar filas entre páginas.
    ///
    /// `tope` acota el número máximo de páginas (guardarraíl contra un dataset que
    /// nunca «termine» por un `$order` no estable). `0` = sin tope.
    pub fn ejecutar_paginado(
        &self,
        c: &Consulta,
        tam_pagina: u64,
        tope_paginas: u64,
    ) -> Result<Vec<Value>, ErrorSocrata> {
        let tam = tam_pagina.max(1);
        let mut todas = Vec::new();
        let mut offset = 0u64;
        let mut paginas = 0u64;
        loop {
            let pagina = c.clone().limite(tam).desde(offset);
            let filas = self.ejecutar(&pagina)?;
            let n = filas.len() as u64;
            todas.extend(filas);
            paginas += 1;
            // Página incompleta ⇒ era la última. Vacía también corta.
            if n < tam {
                break;
            }
            offset += tam;
            if tope_paginas != 0 && paginas >= tope_paginas {
                break;
            }
        }
        Ok(todas)
    }
}

fn tipo(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "número",
        Value::String(_) => "texto",
        Value::Array(_) => "lista",
        Value::Object(_) => "objeto",
    }
}

/// Lee un campo de texto de un objeto Socrata.
pub fn campo_texto<'a>(fila: &'a Value, clave: &str) -> Option<&'a str> {
    fila.get(clave)?.as_str()
}

/// Lee un campo numérico. Socrata suele mandar los números COMO STRING (sobre todo
/// en agregados), así que se intenta string→f64 y, si no, número JSON directo.
pub fn campo_f64(fila: &Value, clave: &str) -> Option<f64> {
    let v = fila.get(clave)?;
    v.as_str()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .or_else(|| v.as_f64())
}

/// Igual que [`campo_f64`] pero entero.
pub fn campo_i64(fila: &Value, clave: &str) -> Option<i64> {
    let v = fila.get(clave)?;
    v.as_str()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .or_else(|| v.as_i64())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::reintentos::Politica;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn arma_la_url_con_las_clausulas() {
        let http = ClienteHttp::nuevo(Duration::from_secs(5), Duration::ZERO, Politica::default()).unwrap();
        let s = Socrata::nuevo("s54a-sgyg", http).con_token("TK");
        let c = Consulta::nueva()
            .select("sum(v) AS s")
            .filtro("depto = 'X'")
            .orden("mes")
            .limite(1000);
        let url = s.url(&c).unwrap();
        assert_eq!(url.path(), "/resource/s54a-sgyg.json");
        let q = url.query().unwrap();
        // POSITIVO: las cláusulas están y van url-encoded (espacios como %20/+).
        assert!(q.contains("%24select=") || q.contains("$select="), "sin $select: {q}");
        assert!(q.contains("depto"), "sin filtro: {q}");
        assert!(q.contains("%24limit=1000") || q.contains("$limit=1000"), "sin limit: {q}");
        assert!(q.contains("app_token=TK"), "sin token: {q}");
    }

    #[test]
    fn campos_como_string_y_como_numero() {
        // Socrata manda strings en agregados; a veces números en columnas typadas.
        let fila: Value = serde_json::json!({"suma": "123.5", "est": "7", "n": 9, "nombre": "Amazonas"});
        // POSITIVO: parsea el string y también el número JSON.
        assert_eq!(campo_f64(&fila, "suma"), Some(123.5));
        assert_eq!(campo_i64(&fila, "est"), Some(7));
        assert_eq!(campo_i64(&fila, "n"), Some(9));
        assert_eq!(campo_texto(&fila, "nombre"), Some("Amazonas"));
        // ROJO: campo ausente o no numérico → None (no un 0 inventado, D20).
        assert_eq!(campo_f64(&fila, "no_existe"), None);
        assert_eq!(campo_f64(&fila, "nombre"), None);
    }

    /// Servidor que sirve `paginas` (una por conexión, en orden) y cuenta las
    /// peticiones. Cada cuerpo es un array JSON.
    fn servidor(paginas: Vec<String>) -> (String, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut n = 0;
            for cuerpo in paginas {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    cuerpo.len(),
                    cuerpo
                );
                let _ = s.write_all(resp.as_bytes());
                n += 1;
            }
            n
        });
        (base, handle)
    }

    fn cliente_contra(base: &str) -> Socrata {
        let http = ClienteHttp::nuevo(Duration::from_secs(5), Duration::ZERO, Politica::instantanea(3)).unwrap();
        // El "host" es el base del servidor local; el dataset se vuelve ruta.
        Socrata::en(base, "prueba", http)
    }

    #[test]
    fn paginado_junta_paginas_hasta_una_incompleta() {
        // Página 1 llena (2 filas), página 2 incompleta (1 fila) → para. 2 peticiones.
        let (base, h) = servidor(vec![
            r#"[{"id":"1"},{"id":"2"}]"#.into(),
            r#"[{"id":"3"}]"#.into(),
        ]);
        let s = cliente_contra(&base);
        let filas = s.ejecutar_paginado(&Consulta::nueva().orden("id"), 2, 100).unwrap();
        assert_eq!(filas.len(), 3);
        assert_eq!(campo_texto(&filas[2], "id"), Some("3"));
        assert_eq!(h.join().unwrap(), 2, "debió pedir exactamente 2 páginas");
    }

    #[test]
    fn paginado_una_pagina_corta_no_pide_mas() {
        // ROJO del control: una sola página incompleta corta a la PRIMERA petición.
        // Si el bucle no mirara `n < tam`, pediría una segunda y el servidor (que
        // sirve 1) dejaría el cliente colgado y el join no daría 1.
        let (base, h) = servidor(vec![r#"[{"id":"1"}]"#.into()]);
        let s = cliente_contra(&base);
        let filas = s.ejecutar_paginado(&Consulta::nueva().orden("id"), 50, 100).unwrap();
        assert_eq!(filas.len(), 1);
        assert_eq!(h.join().unwrap(), 1);
    }

    #[test]
    fn respuesta_que_no_es_lista_es_error_de_forma() {
        // Socrata ante un error devuelve un OBJETO {error:...}, no una lista.
        let (base, h) = servidor(vec![r#"{"error":true,"message":"malo"}"#.into()]);
        let s = cliente_contra(&base);
        let e = s.ejecutar(&Consulta::nueva()).unwrap_err();
        assert!(matches!(e, ErrorSocrata::Forma(_)), "esperaba Forma, vino {e}");
        assert_eq!(h.join().unwrap(), 1);
    }
}
