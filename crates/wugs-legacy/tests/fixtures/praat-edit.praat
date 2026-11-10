# Makes praat-edited.TextGrid from praat-out.TextGrid with real Praat
# (tests/praat.rs). Run from anywhere: paths are relative to this script.
# Tier 1 is "tones" (points), tier 2 is "utt" (intervals).
Read from file: "praat-out.TextGrid"
# bir: end 1.2 -> 1.3, label -> biri
Remove right boundary: 2, 2
Insert boundary: 2, 1.3
Set interval text: 2, 2, "biri"
# a new label in the gap: dört 1.5-1.8
Insert boundary: 2, 1.5
Insert boundary: 2, 1.8
Set interval text: 2, 4, "dört"
# split iki at 2.3 and label the right half
Insert boundary: 2, 2.3
Set interval text: 2, 7, "ki"
# clear üç
Set interval text: 2, 9, ""
# move H to 0.85, add L at 2.0
Remove point: 1, 1
Insert point: 1, 0.85, "H"
Insert point: 1, 2.0, "L"
# a tier that exists only in Praat
Insert interval tier: 3, "notes"
Insert boundary: 3, 1
Set interval text: 3, 1, "note"
Save as text file: "praat-edited.TextGrid"
