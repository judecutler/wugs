# WALS

The World Atlas of Language Structures (WALS) describes 2,662 languages by 192 features, such as "Consonant Inventories" or "Order of Subject, Object and Verb". These two files hold it in the same compact form as `datasets/grambank/`:

- `features.tsv`: one row per feature in WALS's order, with its ID, its name and its values (`1=Small|2=Moderately small|…`). A value's number is written as one character: `1` to `9`, then `a` for 10, `b` for 11 and so on.
- `values.tsv`: one row per Glottocode, with one character per feature in the order of `features.tsv`, or `.` where the feature is not coded for that language, then the WALS code of the language (for its page at wals.info). A few Glottocodes stand for two WALS languages (dialects WALS keeps apart); there the one with more values comes first and the other fills its gaps. WALS languages without a Glottocode are left out.

The wugs desktop app embeds both. The **Typology** panel shows what WALS says about the project's language, beside how the coded languages of the same family answer. Nothing is sent anywhere. Each value's sources and examples stay in WALS, at https://wals.info.

## Source and licence

WALS Online is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Cite it as:

> Dryer, Matthew S. & Haspelmath, Martin (eds.) 2013. The World Atlas of Language Structures Online. Leipzig: Max Planck Institute for Evolutionary Anthropology. (Available online at https://wals.info)

The files were built from the `cldf/` folder of [cldf-datasets/wals](https://github.com/cldf-datasets/wals) (release `v2020.6`, commit `81298aa26d01fdc11b769bcc7fa87ad9e43d7ccd`) by `tools/data/wals.py`. To update the files, run the script on a newer clone and change the version named here.
