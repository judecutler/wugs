# Glottolog languoids

`languoids.tsv` lists every language and dialect in **Glottolog 5.4**. It has one row per languoid, with these columns:

- glottocode
- ISO 639-3 code
- name
- level (`L` for a language, `D` for a dialect)
- family name (or `isolate`)
- macroarea
- the language a dialect belongs to

The wugs desktop app embeds it. The Texts panel's Language row uses it to offer the Glottocode and ISO code for the name you type. Nothing is sent anywhere.

## Source and licence

Glottolog is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Cite it as:

> Hammarström, Harald & Forkel, Robert & Haspelmath, Martin & Bank, Sebastian. 2026. Glottolog 5.4. Leipzig: Max Planck Institute for Evolutionary Anthropology. (Available online at https://glottolog.org)

The table was built from `cldf/languages.csv` in [glottolog/glottolog-cldf](https://github.com/glottolog/glottolog-cldf). Its metadata says the data is Glottolog `v5.4` and the repository version is `v5.3-9-gfbf37d0`. The build script is `tools/data/glottolog.py`. It keeps only languages and dialects (families are dropped), tidies spaces in names, and sorts the rows by glottocode. To update the table, run the script on a newer `languages.csv` and change the version named here.
