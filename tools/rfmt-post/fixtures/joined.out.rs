fn f() {
    let value = registry_with_a_much_longer_name
        .lookup_entry(name)
        .unwrap_or_default()
        .combine_three_values(alpha_value, beta_value, gamma_value);
}
