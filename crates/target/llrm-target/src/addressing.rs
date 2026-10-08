//! A target's address forms, from the `[[address_form]]` tables of its
//! `datalayout.toml`: base + index*scale + displacement, per address size.

use std::collections::BTreeSet;

use llrm_mir::target::AddressForm;

/// The forms `text` describes, native: a form that needs an address-size prefix
/// is one a target states with `prefix_bytes`.
pub fn forms(text: &str) -> Result<Vec<AddressForm>, String> {
    let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
    let Some(rows) = table.get("address_form") else { return Ok(Vec::new()) };
    let rows = rows.as_array().ok_or("address_form is not an array of tables")?;
    rows.iter()
        .map(|row| {
            let bits = row.get("address_bits").and_then(toml::Value::as_integer).ok_or("an address_form needs address_bits")?;
            let scales: BTreeSet<i64> = row
                .get("scales")
                .and_then(toml::Value::as_array)
                .ok_or("an address_form needs scales")?
                .iter()
                .map(|one| one.as_integer().ok_or("a scale is an integer"))
                .collect::<Result<_, _>>()?;
            let extra = row.get("prefix_bytes").and_then(toml::Value::as_integer).unwrap_or(0);
            AddressForm::new(bits / 8, scales, extra, 0, 0, extra > 0, None)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_is_native_unless_it_names_a_prefix() {
        let text = "[[address_form]]\naddress_bits = 32\nscales = [1, 2, 4, 8]\n";
        let forms = forms(text).unwrap();
        assert_eq!(forms.len(), 1);
        assert!(!forms[0].secondary && forms[0].index_width == 4 && forms[0].scales == BTreeSet::from([1, 2, 4, 8]));
        let prefixed = self::forms("[[address_form]]\naddress_bits = 32\nscales = [1]\nprefix_bytes = 1\n").unwrap();
        assert!(prefixed[0].secondary && prefixed[0].extra_bytes == 1);
        assert!(self::forms("x = 1").unwrap().is_empty());
    }
}
