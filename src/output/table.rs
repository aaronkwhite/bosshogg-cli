//! Table rendering — comfy-table wrapper, UTF-8 borders, header bold when TTY.

use comfy_table::{Cell, Row, Table, presets::UTF8_FULL};

use crate::output::color;

/// Print a table with the given header cells and rows to stdout.
pub fn print(headers: &[&str], rows: &[Vec<String>]) {
    let mut t = Table::new();
    t.load_preset(UTF8_FULL);

    let header_cells: Vec<Cell> = headers.iter().map(|h| Cell::new(color::bold(h))).collect();
    t.set_header(Row::from(header_cells));

    for r in rows {
        t.add_row(Row::from(r.iter().map(Cell::new).collect::<Vec<_>>()));
    }

    println!("{t}");
}
