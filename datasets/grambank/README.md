# Grambank

Grambank describes 2,467 languages by 195 grammatical features, such as "Are there definite or specific articles?" or "What is the order of numeral and noun in the NP?". These two files hold it in a compact form:

- `features.tsv`: one row per feature in Grambank's order, with its ID, its question and its codes (`0=absent|1=present`).
- `values.tsv`: one row per language, with its Glottocode and one character per feature in the order of `features.tsv`. The character is the coded value, `?` where the coders could not tell, or `.` where the feature is not coded for that language.

The wugs desktop app embeds both. The Texts panel's **Grammar features** shows what Grambank says about the project's language. When Grambank has not coded the language, it shows how the coded languages of the same family answer each question. Nothing is sent anywhere. The comments, sources and coders behind each value stay in Grambank, and the app links to the language's page at https://grambank.clld.org.

## Source and licence

Grambank is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Cite it as:

> Skirgård, Hedvig, Hannah J. Haynie, Damián E. Blasi, Harald Hammarström, Jeremy Collins, Jay J. Latarche, Jakob Lesage, Tobias Weber, Alena Witzlack-Makarevich, Sam Passmore, et al. 2023. Grambank reveals the importance of genealogical constraints on linguistic diversity and highlights the impact of language loss. *Science Advances* 9(16). eadg6175. https://doi.org/10.1126/sciadv.adg6175

The files were built from the `cldf/` folder of [grambank/grambank](https://github.com/grambank/grambank) (Grambank v1.0, repository version `v1.0.2`, commit `37f73da55cf8b426c82383f46a972bc59ce6cf76`) by `tools/data/grambank.py`. It keeps every value as coded and drops comments, sources and coders. To update the files, run the script on a newer clone and change the version named here.
