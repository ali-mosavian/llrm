async fn g<'a>(s: &'a Store) -> Result<Vec<u8>, Error> {
    let rows = s
        .query::<Row>("(a) -- '{")
        .await?
        .into_iter()
        .try_fold(
            Vec::new(),
            |mut all, row: Row<'a>| {
                all.extend(row.bytes.iter().map(|b| if *b == b'(' { b')' } else { *b }));
                Ok::<_, Error>(all)
            },
        )?;
    let text = names
        .iter()
        .map(
            |name| {
                let line = format!(
                    "{name}: {} {}",
                    r#"a "quoted" (paren"#,
                    "
  continued ("
                );
                line + "x"
            },
        );
    Ok(rows)
}
