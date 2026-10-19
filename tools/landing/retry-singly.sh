# retry_singly LOG N: true when the batch refused PR N only for not merging with the others; it may land alone
retry_singly() { grep -qE "^([0-9:]+ )?#$2: does not merge with the others" "$1"; }
