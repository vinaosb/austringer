---
name: flesch-kincaid
description: >
  Audit and revise text readability using Flesch-Kincaid metrics. Computes Flesch Reading Ease
  and Flesch-Kincaid Grade Level scores, flags difficult sentences, and rewrites text to a
  target grade range. Supports English, Portuguese, Spanish, French, German, Italian, and Dutch.
  Includes specialized modes for legal/technical documents where vocabulary precision must be
  preserved. Use when user asks to check readability, grade level, reading ease, simplify text,
  make text easier/harder, "what grade level is this", or invokes /readability.
---

# Flesch-Kincaid Readability Audit & Revision

Measure and rewrite text so it lands in a target reading level. This skill has two modes:
**Audit** (score existing text) and **Revision** (rewrite to a target grade). For precision-critical
text (legal, technical, regulatory), use the **Legal/Technical Mode** (section 8) which preserves
vocabulary while restructuring sentences.

---

## 1. Formulas by Language

Always detect the text's language first, then apply the matching formula. The structure is
identical across languages. Only the constants change:

```
Reading Ease = A - (B x ASL) - (C x ASW)
```

Where:
- **ASL** = Average Sentence Length = total words / total sentences
- **ASW** = Average Syllables per Word = total syllables / total words
- **A, B, C** = language-specific constants (below)

| Language | A | B | C | Notes |
|----------|---|---|---|-------|
| **English** (Flesch 1948) | 206.835 | 1.015 | 84.6 | Gold standard |
| **Portuguese** (Flesch Adaptado) | 248.835 | 1.015 | 84.6 | Higher intercept compensates for more syllables/word in Portuguese |
| **Spanish** (Fernandez Huerta) | 206.84 | 1.02 | 60.0 | C lowered because Spanish words are longer |
| **French** (Kandel-Moles) | 207.0 | 1.015 | 73.6 | C adjusted for French morphology |
| **German** (Amstad) | 180.0 | 1.015 | 58.4 | Both A and C lowered for German compound words |
| **Italian** (Flesch-IT) | 217.0 | 1.3 | 60.0 | Approximation. Italian formulas are less standardized. |
| **Dutch** (Douma) | 206.835 | 0.77 | 84.6 | B adjusted for Dutch sentence structure |

### English Grade Level Formula (Flesch-Kincaid Grade Level, Kincaid et al. 1975)

```
FKGL = (0.39 x ASL) + (11.8 x ASW) - 15.59
```

This gives a direct US grade-level number (e.g., 8.2 = 8th grade, month 2).

> **For non-English languages**: The FKGL formula above is calibrated for English only.
> For other languages, compute the Reading Ease score, then map it to a grade level using
> the table in section 2. Do NOT apply the English FKGL formula to non-English text. It
> will produce inaccurate results.

---

## 2. Score Interpretation & Grade Mapping

### Reading Ease to Grade Level (all languages)

| Reading Ease | Reading Level | US Grade | Age (years) | Description |
|:---:|---|:---:|:---:|---|
| 90-100 | Very easy | 5th | 10-11 | Easily understood by an average 11-year-old |
| 80-90 | Easy | 6th | 11-12 | Conversational English for consumers |
| 70-80 | Fairly easy | 7th | 12-13 | Fairly easy to read |
| 60-70 | Standard | 8th-9th | 13-15 | Plain English. ~70 is the target for most general writing |
| 50-60 | Fairly difficult | 10th-12th | 15-18 | Moderate complexity. Professional and authoritative. |
| 30-50 | Difficult | College | 18-22 | Academic, professional. Hard for many readers. |
| 0-30 | Very difficult | College graduate | 22+ | Best for specialists. Very hard for general public. |

### Professional Context Targets

Choose the target based on audience and document type:

| Context | Grade Target | Reading Ease | Tone | Examples |
|---|:---:|:---:|---|---|
| General public | 7-9 | 60-70 | Conversational, clear | Consumer content, blogs, public notices, marketing |
| Professional/authoritative | 10-12 | 50-60 | Professional, authoritative | B2B proposals, executive summaries, policies |
| Legal for general public | 8-10 | 60-70 | Clear, accessible | Contracts, disclosures, consumer terms |
| Legal for professionals | 10-12 | 50-60 | Precise, authoritative | Statutes, constitutions, regulations, treaties |
| Academic/specialist | 13-15 | 30-50 | Academic, dense | Whitepapers, scientific journals, internal audits |
| Exclusionary (avoid) | 16+ | 0-30 | Bureaucratic | Alienates readers. Never intentional. |

> **Key insight**: Going above Grade 12 does NOT make writing sound more professional.
> It only makes it harder to read. Even highly educated audiences prefer content below
> their capacity. Grade 10-12 is the sweet spot for professional and legal text: it sounds
> sophisticated while remaining accessible to any literate adult.

### Reverse: Target Grade to Target Reading Ease Range

Use this when rewriting to a target grade:

| Target Grade | Target Reading Ease | Writing Style |
|:---:|:---:|---|
| 5th (age 10-11) | 90-100 | Short sentences (under 10 words). Simple vocabulary. |
| 6th (age 11-12) | 80-90 | Conversational. Common words only. |
| 7th (age 12-13) | 70-80 | Fairly simple. Some longer words OK. |
| 8th-9th (age 13-15) | 60-70 | Standard plain language. Best default for public-facing text. |
| 10th-12th (age 15-18) | 50-60 | Moderate complexity. Trade/professional vocabulary OK. |
| College (age 18+) | 30-50 | Academic register. Complex sentences. |
| Graduate (age 22+) | 0-30 | Specialist jargon. Dense sentence structure. |

---

## 3. Formula Sensitivity & Lever Selection

Before revising, understand which lever (sentence length vs word length) has the most impact.
This avoids wasted effort on the wrong axis.

### The Math

```
FKGL = (0.39 x ASL) + (11.8 x ASW) - 15.59
```

The ASW coefficient (11.8) looks 30x larger than the ASL coefficient (0.39). But consider
the realistic ranges of each variable:

| Variable | Typical range | Spread | Grade-level impact of full spread |
|---|---|---|---|
| ASL (words/sentence) | 5 to 50+ | 45+ points | 0.39 x 45 = **17.6 grade levels** |
| ASW (syllables/word) | 1.4 to 2.0 | 0.6 points | 11.8 x 0.6 = **7.1 grade levels** |

**ASL is the dominant lever by roughly 2.5x.** Sentence length has more practical impact
than word length, even though the coefficient is smaller. This is because ASL varies far more
than ASW in real text.

### Lever Selection Decision Table

| Current ASL | Current ASW | Primary lever | Strategy |
|:---:|:---:|---|---|
| Over 20 | any | **Sentence length** | Split long sentences aggressively. This alone will drop 2-4 grades. |
| 15-20 | Over 1.8 | **Sentence length first** | Split sentences, then simplify vocabulary if still above target. |
| 15-20 | Under 1.7 | Sentence length | Split sentences. Vocabulary is already simple. |
| Under 15 | Over 1.8 | **Vocabulary** | Replace multi-syllable words with shorter synonyms. |
| Under 12 | Under 1.6 | Already accessible | Minimal revision needed. Check individual problem sentences. |

### Worked Example

A legal document scores Grade 14.6 with ASL = 19.8 and ASW = 1.90:

- Reducing ASL from 19.8 to 13.3 (splitting sentences): drops Grade by 0.39 x 6.5 = **2.5 grades**
- Reducing ASW from 1.90 to 1.70 (simplifying vocabulary): drops Grade by 11.8 x 0.20 = **2.4 grades**

Both levers are equally powerful HERE because the ASW change is large (0.20). But reducing ASW
by 0.20 requires replacing most multi-syllable words, which is often impossible in legal text.
Reducing ASL by 6.5 only requires splitting sentences, which is always possible.

**Conclusion for legal/technical text: always start with ASL (sentence splitting).**

---

## 4. Syllable Counting Rules

Accurate syllable counts are the foundation of accurate scores. Rules vary by language.

### English
- Count **vowel sounds**, not vowel letters. Each contiguous group of vowel sounds = 1 syllable.
- Silent trailing `e` does **not** count (`"made"` = 1, `"care"` = 1, `"recipe"` = 3).
- `y` at end of word counts as a vowel (`"happy"` = 2).
- Common suffixes: `-tion`, `-sion`, `-ture`, `-eous` = 1 syllable each.
- Acronyms/initialisms: count 1 syllable per letter that has a vowel sound (`"NASA"` = 2, `"FBI"` = 3).
- Numbers written as digits: read them out and count (`"2024"` = "twenty twenty-four" = 4).
- Contractions: count as if expanded (`"don't"` = 1, `"can't"` = 1, `"I'm"` = 1).

### Portuguese (Brazilian / European)
- Count vowel **groups** separated by consonants. Each nucleus = 1 syllable.
- Diphthongs (`ai`, `ei`, `eu`, `ou`, `ao`, `ae`, `oe`) = 1 syllable.
- Trailing `-m` (`sim`, `tambem`) does not create a new syllable. It nasalizes.
- Accented vowels mark stressed nuclei.
- Example: `"computacao"` = com-pu-ta-cao = 4 syllables.

### Spanish
- Same logic as Portuguese: vowel groups separated by consonants.
- Strong vowels (`a`, `e`, `o`) + weak vowels (`i`, `u`) form diphthongs = 1 syllable.
- Two strong vowels in contact = 2 syllables.
- Accented vowels force a syllable break.
- Example: `"internacionalizacion"` = in-ter-na-cio-na-li-za-cion = 8 syllables.

### French
- Count **spoken** syllables, not written vowels.
- Silent word-final `e` = 0 syllables (`"table"` = 1: "tabl").
- `-ent` plural verb ending = silent (0 syllables).
- Mute `h` does not break syllables. Aspirate `h` does.
- Liaisons can merge or create syllables across word boundaries.

### German
- Count vowel groups. Umlauts are vowels.
- Compound words: count syllables across the whole compound.
- `ie` = 1 syllable (long `i` sound) unless split by consonant cluster.
- Example: `"Entschuldigung"` = Ent-schul-di-gung = 4 syllables.

### Italian
- Almost phonetic: every vowel is pronounced. Count vowel sounds.
- Doubled consonants (`ll`, `rr`, `tt`) don't add syllables.
- `"gn"` (as in `"gnocchi"`) = 1 consonant cluster, doesn't split syllable.
- Example: `"impossibile"` = im-pos-si-bi-le = 5 syllables.

### Dutch
- Count vowel groups. Diphthongs (`ij`, `ui`, `eu`, `oe`, `ou`) = 1 syllable.
- Compound words: count across the full word.
- Example: `"verantwoordelijkheid"` = ver-ant-woor-de-lijk-heid = 6 syllables.

---

## 5. Audit Protocol (score existing text)

Run this when the user wants to **measure** readability without rewriting.

### Step 1: Gather the text
- If the user pastes text, use it directly.
- If the user gives a file path, read the file.
- If the user gives a URL, fetch and extract the text content.

### Step 2: Detect language
- Identify the dominant language. If mixed (e.g., English text with Portuguese quotes),
  score the dominant language and note the mix.
- If language is not in the supported list (section 1), fall back to **English** formula
  and add a caveat: *"Language X is not in the supported list. English formula used as
  approximation. Scores may be less accurate."*

### Step 3: Compute raw counts
Count these across the entire text sample:

| Metric | How to count |
|---|---|
| **Sentences** | Split on `.`, `!`, `?`, `;` (and language-specific equivalents). Count at least 1 sentence even if no terminal punctuation. |
| **Words** | Whitespace-delimited tokens. Strip punctuation. Count contractions and hyphenated words as 1 word each. Numbers written as digits count as words. |
| **Syllables** | Use the language rules in section 4. Sum across all words. |

> **Minimum sample size**: For meaningful scores, use **at least 100 words / 5 sentences**.
> If the sample is shorter, compute the score but flag: *"Sample is small (N words).
> Score is less reliable."*

### Step 4: Apply formula
Compute ASL, ASW, then apply the formula for the detected language.

### Step 5: Report results

Present results in this format:

```
## Readability Audit

**Language:** English
**Sample size:** 247 words, 14 sentences

### Scores
| Metric | Value | Interpretation |
|---|---|---|
| Reading Ease | 62.3 | Standard (8th-9th grade) |
| Flesch-Kincaid Grade | 8.4 | US grade 8, month 4 |
| Avg. sentence length | 17.6 words | Slightly long |
| Avg. syllables/word | 1.6 | Moderate |

### Lever analysis
Primary lever: **Sentence length** (ASL = 17.6 is above the 15-word threshold).
Reducing ASL to 12 would drop the grade by approximately 2.2 levels.

### Sentence-level flags
| # | Sentence (truncated) | Words | Issue |
|---|---|---|---|
| 3 | "Despite the implementation of comprehensive regulatory frameworks..." | 34 | Very long. Consider splitting. |
| 7 | "The multidisciplinary approach facilitates optimization..." | 22 | Complex vocabulary (11 multi-syllable words) |

### Summary
This text reads at a **8th-grade** level. It is accessible to most adults.
Target for public-facing content is typically grade 7-9 (Reading Ease 60-80).
```

### When to flag sentences
- **Word count > 25**: "Very long"
- **Word count > 20 AND > 50% multi-syllable words**: "Dense"
- **Any sentence dragging the average up**: list top 3 offenders

### Per-section analysis (for multi-section documents)

For documents with clear section structure (legal statutes, technical specs, manuals),
score **each section independently** and sort worst-first. The document-wide average
hides problem sections.

Report:
```
### Per-section breakdown (worst 10 of 45 sections)
  Grade  Ease  Words  Sent  Section
  16.7    2.8     62     4  Section 10. Standing to Challenge
  16.4    9.8     55     3  Section 12. First Legislature Meeting
  ...
Summary: 32 sections above target, 13 at or below target.
```

Use per-section analysis to guide revision: fix the worst sections first for maximum
impact with minimum effort.

---

## 6. Revision Protocol (rewrite to target grade)

Run this when the user wants text **rewritten** to a specific grade level.

### Step 1: Determine target
- Ask for or confirm the target grade range (see section 2 for the menu).
- If the user says "simplify" or "make it easier" without a number, default to
  **grade 7-8 (Reading Ease 70-80)** for general audiences.
- If the user says "make it more sophisticated," default to **grade 12+ (Reading Ease 30-50)**.
- For legal documents, default to **grade 10-12 (Reading Ease 50-60)** unless the user
  specifies otherwise.

### Step 2: Audit the source text first
Always run the full Audit Protocol (section 5) on the source so you can show before/after.
Check ASL and ASW separately to determine which lever to pull (section 3).

### Step 3: Select revision mode

**General mode** (section 7 below): For prose where vocabulary can be freely simplified.
Use for blog posts, marketing copy, consumer documentation, emails.

**Legal/Technical mode** (section 8 below): For documents where vocabulary precision is
mandatory. Use for statutes, contracts, regulations, technical specifications, patents.

### Step 4: Apply revision strategies

Follow the strategies for the selected mode (sections 7 or 8).

### Step 5: Preserve meaning (CRITICAL)
- **Never** change the factual content, claims, or data.
- **Never** remove information to lower the score. Rephrase it instead.
- **Never** change the author's intent or stance.
- **Preserve** all named entities, numbers, dates, citations, and proper nouns.
- **Preserve** the text's structure (headings, lists, paragraphs) unless asked otherwise.
- **Match the register**: a legal text simplified to 8th grade should still sound professional,
  not childish.

### Step 6: Re-score the revision
After rewriting, run the Audit Protocol on the **revised** text. Present a before/after comparison:

```
## Revision Complete

**Target:** Grade 10-12 (Reading Ease 50-60)

### Before to After
| Metric | Before | After | Change |
|---|---|---|---|
| Reading Ease | 52.1 | 58.3 | +6.2 |
| Flesch-Kincaid Grade | 11.2 | 10.1 | -1.1 |
| Avg. sentence length | 22.3 | 14.1 | -8.2 |
| Avg. syllables/word | 1.8 | 1.7 | -0.1 |

### Revised text
[full rewritten text here]
```

If the revised text does not hit the target range, iterate (revise again, re-score) until
it lands in range or until 3 iterations, then report the closest achieved.

---

## 7. General Revision Strategies

For prose where vocabulary can be freely simplified. Use for blog posts, marketing, consumer
documentation, emails, and most non-technical content.

### To LOWER the grade level (simplify):

1. **Shorten sentences**: Break any sentence over the threshold into two or more.
   - Target Grade 8: split at 12 words
   - Target Grade 10: split at 15 words
   - Target Grade 12: split at 18 words
   - General rule: one idea per sentence.

2. **Simplify vocabulary**: Replace 3+ syllable words with 1-2 syllable synonyms.
   - `"utilize"` becomes `"use"`, `"demonstrate"` becomes `"show"`, `"facilitate"` becomes `"help"`.
   - `"pertaining to"` becomes `"about"`, `"in order to"` becomes `"to"`.
   - `"due to the fact that"` becomes `"because"`.

3. **Active voice**: Convert passive constructions to active.
   - `"The report was written by the team"` becomes `"The team wrote the report."`
   - Passive voice is weaker, less clear, and adds words.

4. **Cut filler and clutter**: Remove unnecessary adjectives, adverbs, and transitional phrases.
   - `"It should be noted that"` becomes nothing (just state the fact).
   - `"At this point in time"` becomes `"now"`.

5. **Reduce subordination**: Replace nested clauses with separate sentences.
   - `"The system, which was designed by engineers who specialized in safety, failed"` becomes
     `"Engineers specialized in safety designed the system. It failed."`

6. **Concrete examples**: Replace abstract statements with concrete instances.

### To RAISE the grade level (sophisticate):

1. **Varied sentence length**: Mix short and long sentences. Use periodic and balanced structures.
2. **Advanced vocabulary**: Use precise, multi-syllable words where they add nuance.
3. **Complex syntax**: Use subordinate clauses, relative clauses, appositives.
4. **Academic register**: Adopt formal tone, domain-appropriate terminology.
5. **Passive where conventional**: Academic/professional writing often uses passive.

> **Note**: Professional writing is defined by precision and structure, not by long sentences
> or obscure words. You can maintain a Grade 10-12 tone using accurate nouns and verbs,
> 15-20 word sentences, and active voice. Do not equate complexity with professionalism.

---

## 8. Legal/Technical Document Mode

For documents where vocabulary precision is mandatory: statutes, constitutions, contracts,
regulations, technical specifications, patents, treaties. In these documents, **sentence
structure is the only safe lever.**

### Core Principle

Never simplify vocabulary in legal/technical text. Defined terms ("Constitutional Court",
"non-aggression principle", "Fiscal Council") carry precise legal meaning. Replacing them
with simpler synonyms changes the law. Instead, restructure sentences to reduce average
sentence length while preserving every word that carries meaning.

### Strategies (in priority order)

#### Strategy 1: Split long sentences (PRIMARY TOOL)

Break any sentence over the target threshold at natural clause boundaries.

- Target Grade 12 for legal text: split at **18 words**
- Target Grade 10 for consumer legal text: split at **15 words**

**Before** (Grade 18, 34 words):
> "In the event that any provision of this Agreement is held to be invalid or unenforceable,
> such provision shall be modified to the minimum extent necessary to make it valid, and the
> remaining provisions shall remain in full force and effect."

**After** (Grade 9, three sentences):
> "If a court finds any part of this agreement invalid, that part will be changed. The change
> shall be the minimum needed to make it valid. The rest of the agreement stays as it is."

Break at these natural boundaries:
- Commas separating independent clauses
- Semicolons (always convert to periods)
- Conjunctions: `"and"`, `"or"`, `"but"` when joining independent clauses
- Transitional phrases: `"provided that"`, `"whereas"`, `"except that"`, `"unless"`, `"except when"`
- Relative clauses starting with `"which"` or `"that"` when non-restrictive

#### Strategy 2: Convert inline lists to lettered subsections

When a sentence contains an inline list of conditions, requirements, or items, convert it to
a structured subsection format. Each item becomes its own line.

**Before** (Grade 16, 38 words, 1 sentence):
> "The Council shall review all applications within thirty days, conduct public hearings
> prior to rendering a decision, issue written decisions containing findings of fact and
> conclusions of law, and maintain detailed records of all proceedings for a period of
> not less than ten years."

**After** (Grade 11, 5 sentences):
> "The Council shall perform the following duties:
> (a) review all applications within thirty days;
> (b) conduct public hearings prior to rendering a decision;
> (c) issue written decisions containing findings of fact and conclusions of law; and
> (d) maintain detailed records of all proceedings for not less than ten years."

This technique is extremely effective because it converts one long sentence into many short
ones without changing any word.

#### Strategy 3: Break conditional chains

Legal text often chains conditions: `"X shall apply, provided that Y, unless Z, except in
cases where W"`. Break these into separate declarative sentences.

**Before** (Grade 17, 42 words, 1 sentence):
> "The Commission may issue subpoenas requiring the production of documents, provided that
> the Commission has reasonable grounds to believe a violation has occurred, unless the
> documents sought are protected by attorney-client privilege, except in cases where a
> court of competent jurisdiction has ordered disclosure."

**After** (Grade 10, 4 sentences):
> "The Commission may issue subpoenas requiring the production of documents. The Commission
> must have reasonable grounds to believe a violation has occurred before issuing a subpoena.
> Documents protected by attorney-client privilege are exempt. A court of competent
> jurisdiction may order disclosure of otherwise exempt documents."

#### Strategy 4: Isolate citations and cross-references

Case citations, statutory references, and long cross-references inflate sentence length and
syllable count. Move them to separate sentences or parentheticals so the core rule remains
readable.

**Before** (Grade 16, 28 words):
> "Any person aggrieved by a decision of the Commission may appeal to the Constitutional Court
> pursuant to Section 15 of the Criminal Procedure Act (Statute 2) within thirty days."

**After** (Grade 11, 2 sentences):
> "Any person aggrieved by a decision of the Commission may appeal to the Constitutional Court.
> The appeal must be filed within thirty days under Section 15 of the Criminal Procedure Act
> (Statute 2)."

#### Strategy 5: Replace archaic "lawyerisms" with direct verbs

Some legal vocabulary is archaic rather than precise. These can be safely replaced:

| Archaism | Plain equivalent |
|---|---|
| `"bequeath"` | `"give"` |
| `"cease"` | `"stop"` |
| `"pertaining to"` | `"about"` or `"for"` |
| `"in the event that"` | `"if"` |
| `"subsequent to"` | `"after"` |
| `"prior to"` | `"before"` |
| `"in accordance with"` | `"under"` |
| `"notwithstanding"` | `"despite"` |
| `"hereinafter"` | `"below"` or omit |
| `"shall be required to"` | `"shall"` |

> **Caution**: Only replace words that are genuinely archaic. Do NOT replace defined terms
> (capitalized terms like "Constitutional Court"), technical terms with specific legal
> meanings ("fiduciary duty", "quantifiable harm"), or terms that appear in definitions
> sections.

### What NOT to do in legal/technical mode

- Do NOT replace defined terms with simpler synonyms. "Constitutional Court" is NOT "court."
- Do NOT remove precision qualifiers. "Not less than thirty days" is NOT "about a month."
- Do NOT merge distinct conditions into vague single sentences.
- Do NOT remove numbers, thresholds, penalties, or timelines.
- Do NOT change cross-references or citations.
- Do NOT add new content, commentary, or explanatory text.
- Do NOT change operative verbs ("shall", "may", "must not") as they carry legal meaning.

---

## 9. Multi-Document & Batch Mode

### Batch Audit

If the user provides multiple texts, files, or a directory:
1. Score each one independently.
2. Present a summary table:

```
## Batch Audit Summary

| File | Language | Words | Reading Ease | Grade | Status |
|---|---|---|---|---|---|
| intro.md | English | 340 | 68.2 | 8.1 | On target |
| api-doc.md | English | 1200 | 41.5 | 12.8 | Too complex |
| tutorial.md | Portuguese | 890 | 72.1 | 7.5 | On target |
```

3. Flag any file outside the target band.

### Systematic Revision Workflow (for multi-document projects)

For projects requiring revision of multiple large documents to a target grade:

1. **Baseline audit**: Score all documents. Record ASL and ASW for each.
2. **Prioritize**: Sort documents by gap-to-target (largest gap first).
3. **Lever analysis**: For each document, determine primary lever (section 3).
   - ASL over 20: sentence splitting is the main tool.
   - ASW over 1.8: vocabulary simplification helps (if mode allows).
4. **Per-section breakdown**: For each document, score sections independently.
   Focus revision on the worst sections first for maximum impact.
5. **Revise in parallel**: If documents are independent (different files), revise them
   simultaneously. For a single large file, split by section range and revise in chunks.
6. **Re-score after each pass**: Verify improvement. Iterate on any sections still above target.
7. **Final verification**: Score all documents. Confirm all hit the target.

---

## 10. Reference Implementation

A Python script for measuring F-K scores is available at:
`C:\Users\vsbb1\AppData\Local\Temp\opencode\fk_score.py`

### Usage

```bash
# Score all draft documents in current directory
python "C:\Users\vsbb1\AppData\Local\Temp\opencode\fk_score.py"

# Score a specific document with per-section breakdown
python "C:\Users\vsbb1\AppData\Local\Temp\opencode\fk_score.py" constitution --worst 30

# Arguments:
#   <docname>   Document name prefix (e.g., "constitution", "statutes", "annexes")
#   --worst N   Show top N worst sections by grade level
#   --target T  Set target grade (default: 12.0)
```

### Output

The script reports per-document and per-section scores including:
- Reading Ease score and label
- Flesch-Kincaid Grade Level
- Word count, sentence count, syllable count
- Average words per sentence (ASL) and syllables per word (ASW)
- Gap to target grade
- Top N worst sections with line numbers for navigation

> If the script does not exist, recreate it. The core algorithm:
> 1. Strip markdown formatting (headers, bold, links, code blocks).
> 2. Split into sections by markdown headers (`##` or `###`).
> 3. For each section, count sentences (split on `.!?;`), words (whitespace tokens),
>    and syllables (vowel-group heuristic).
> 4. Apply FKGL formula: `(0.39 x ASL) + (11.8 x ASW) - 15.59`.
> 5. Report per-section and document-wide aggregates.

---

## 11. Edge Cases & Rules

- **Code blocks**: Exclude from scoring (they are not prose). Score only the surrounding text.
- **Headings**: Score them as part of the text (they are usually short sentences). Exclude
  from per-section analysis if they skew results.
- **Tables/data**: Exclude from prose scoring. Note separately.
- **Mixed-language text**: Score the dominant language. Note the mix.
- **Very short text** (under 100 words): Score but flag as less reliable.
- **Very long text** (over 5000 words): Score per-section and report the distribution.
  For revision work, use per-section scoring (not random sampling). The document-wide
  average hides problem sections. Revise worst sections first.
- **Cross-references and citations**: Include in scoring (they count as words and syllables).
  For revision, isolate them using Strategy 4 in section 8.
- **Defined terms** (legal/technical): Include in scoring. They inflate ASW but cannot be
  changed. Compensate by reducing ASL through sentence splitting.
- **Non-Latin scripts**: Flesch-Kincaid is **not applicable**. These languages do not use
  spaces between words, Latin syllables, or Western sentence boundaries. Applying Flesch-Kincaid
  to non-Latin text produces meaningless results. Use the language-specific formulas below.

  **Chinese (logographic):** Words are not space-separated. "Syllables" do not exist in the
  Latin sense. Readability is driven by character complexity and rarity.
  - Metric: Chinese Readability Index
  - What makes it difficult: high density of rare, multi-stroke characters
  - Measurement: character stroke counts and frequency of rare versus everyday characters
  - Plain-language fix: use high-frequency, everyday characters

  **Japanese (mixed script):** Uses three scripts simultaneously (Hiragana, Katakana, Kanji).
  Readability is driven by the ratio of scripts.
  - Metric: Hasebe-Lee Readability Index
  - What makes it difficult: high concentration of Kanji (which encode complex root ideas)
  - Measurement: ratio of Kanji characters to Hiragana characters
  - Plain-language fix: increase Hiragana usage for structural and grammatical words

  **Arabic (root-based abjad):** Words are built from three- or four-letter consonant roots
  with prefixes and suffixes. Readability is driven by word complexity after removing affixes.
  - Metric: OSMAN Formula or Automatic Arabic Readability Index (AARI)
  - What makes it difficult: long words with many letters added to the root
  - Measurement: flag words with more than six letters after removing common prefixes (such as
    the definite article "al")
  - Plain-language fix: break long multi-clause sentences and use shorter root forms

  **Hebrew:** Use the Hebrew Readability Formula (Cohen et al.).
  **Hindi:** Syllable-based formulas exist, calibrated for Devanagari script.
  **Korean:** Uses Hangul (alphabetic syllabary). Space-separated. Adapted Flesch formulas
  exist but are less standardized.

  **Universal rule for non-Latin scripts**: Regardless of the script, plain-language principles
  still apply to sentence length and structure. Shorter sentences with simpler vocabulary are
  easier to read in every language. Track average sentence character count and common-to-rare
  vocabulary ratios to ensure accessibility.

---

## 12. Output Format Standards

### Audit output
Always include:
1. Language detected
2. Sample size (words + sentences)
3. Reading Ease score + interpretation
4. Grade level (FKGL for English; mapped for others)
5. ASL and ASW
6. Lever analysis (which variable to change and by how much)
7. Flagged sentences (if any)
8. One-line summary

### Revision output
Always include:
1. Target grade range
2. Revision mode used (general or legal/technical)
3. Before/after comparison table
4. Full revised text
5. Confirmation that meaning was preserved

---

## 13. When NOT to Use This Skill

- **Creative writing critique** (use general feedback, not readability scoring)
- **Translation quality** (different concern; FK does not measure translation accuracy)
- **SEO analysis** (different tooling)
- **Grammar/spelling checking** (different tooling)
- **Technical accuracy review** (different tooling)
- **Tone or voice analysis** (different tooling)

This skill measures **readability** (how easy text is to read) and revises text to hit a
target grade level. Nothing else.
