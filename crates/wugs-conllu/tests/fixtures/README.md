# CoNLL-U fixtures

`sample.conllu` is written by hand for these tests (synthetic, no corpus
data): two documents (`# newdoc`), multi-word tokens, an empty node with
enhanced dependencies, `SpaceAfter=No`, punctuation, a translation comment
and a sentence with no `# text`. It is in the order CoNLL-U asks for, so it
must come back from an import and an export byte for byte.
