//! Renderizado determinista de tablas de texto para las salidas de `bench`.
//!
//! Lógica pura: recibe las cabeceras y las filas ya calculadas y devuelve el
//! texto de la tabla. La anchura de cada columna es el máximo entre su cabecera
//! y sus celdas, de modo que la salida es estable y comprobable en tests.

/// Renderiza una tabla de texto con columnas alineadas a la izquierda.
///
/// # Formato
///
/// Cada columna se separa con dos espacios y se rellena con espacios hasta la
/// anchura máxima de esa columna. Tras la cabecera se inserta una línea de
/// guiones del mismo ancho. No se emite espacio sobrante al final de las líneas
/// ni salto de línea final.
///
/// Las filas con menos celdas que cabeceras se completan con celdas vacías.
#[must_use]
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let columns = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate().take(columns) {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(render_row(
        &headers.iter().map(|h| (*h).to_string()).collect::<Vec<_>>(),
        &widths,
    ));
    lines.push(
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  "),
    );
    for row in rows {
        let cells = (0..columns)
            .map(|index| row.get(index).cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        lines.push(render_row(&cells, &widths));
    }
    lines.join("\n")
}

/// Compone una línea con las celdas alineadas a la izquierda y sin espacios finales.
fn render_row(cells: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        let cell = cells.get(index).map_or("", String::as_str);
        line.push_str(cell);
        let padding = width.saturating_sub(cell.chars().count());
        line.extend(std::iter::repeat_n(' ', padding));
    }
    line.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headers_rule_and_rows() {
        let table = render_table(
            &["model", "tok/s"],
            &[
                vec!["a:latest".into(), "41.84".into()],
                vec!["bb:latest".into(), "12.00".into()],
            ],
        );

        let expected = "\
model      tok/s
---------  -----
a:latest   41.84
bb:latest  12.00";
        assert_eq!(table, expected);
    }

    #[test]
    fn aligns_columns_to_widest_cell() {
        let table = render_table(
            &["m", "context"],
            &[vec!["very-long-model:latest".into(), "32768".into()]],
        );

        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[0], "m                       context");
        assert_eq!(lines[1], "----------------------  -------");
        assert_eq!(lines[2], "very-long-model:latest  32768");
    }

    #[test]
    fn pads_short_rows_with_empty_cells() {
        let table = render_table(&["a", "b", "c"], &[vec!["1".into()]]);

        assert_eq!(table.lines().next(), Some("a  b  c"));
        assert_eq!(table.lines().last(), Some("1"));
    }

    #[test]
    fn renders_only_header_and_rule_without_rows() {
        let table = render_table(&["model", "tok/s"], &[]);
        assert_eq!(table, "model  tok/s\n-----  -----");
    }
}
