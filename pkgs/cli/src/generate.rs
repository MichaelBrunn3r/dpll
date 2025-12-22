use std::{
    error::Error,
    io::{self, BufWriter, Write},
};

pub fn generate(num_holes: usize) -> Result<(), Box<dyn Error>> {
    let num_pigeons = num_holes + 1;
    let num_vars = num_pigeons * num_holes;

    let num_clauses_pos = num_pigeons;
    let num_clauses_neg = num_holes * (num_pigeons * (num_pigeons - 1) / 2);
    let num_total_clauses = num_clauses_pos + num_clauses_neg;

    let stdout = io::stdout();
    let handle = stdout.lock();
    let mut writer = BufWriter::new(handle);

    writeln!(
        writer,
        "c Pigeonhole Principle PHP {} pigeons in {} holes",
        num_pigeons, num_holes
    )?;
    writeln!(writer, "p cnf {} {}", num_vars, num_total_clauses)?;

    // Every pigeon in at least one hole
    for p in 1..=num_pigeons {
        for h in 1..=num_holes {
            let var = (p - 1) * num_holes + h;
            write!(writer, "{} ", var)?;
        }
        writeln!(writer, "0")?;
    }

    // No two pigeons in the same hole
    for h in 1..=num_holes {
        for p1 in 1..=num_pigeons {
            for p2 in (p1 + 1)..=num_pigeons {
                let var1 = (p1 - 1) * num_holes + h;
                let var2 = (p2 - 1) * num_holes + h;
                writeln!(writer, "-{} -{} 0", var1, var2)?;
            }
        }
    }

    // Footer
    writeln!(writer, "%")?;
    writeln!(writer, "0")?;

    writer.flush()?;
    Ok(())
}
