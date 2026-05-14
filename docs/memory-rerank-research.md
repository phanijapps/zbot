# Research: Reranking Approaches for zbot Memory Recall

## Executive summary

After surveying ~30 papers, blog posts, and production system docs, the practical landscape for memory/RAG reranking sorts into three tiers: (1) **fast, well-understood baselines** — RRF, MMR, query expansion, and a small cross-encoder — which together usually capture 60–80% of the available quality gain; (2) **larger cross-encoder rerankers** (BGE-reranker-v2, MS-MARCO MiniLM, Jina v2) which add ~3–7 NDCG@10 points over hybrid retrieval at 50–200 ms latency; and (3) **listwise LLM rerankers + classifier-routed pipelines** which top the leaderboards (RankGPT/GPT-4 at NDCG@10 ≈ 0.74 on TREC-DL averages) but at sharply higher cost. For zbot specifically — a single-user desktop assistant where recall noise (30+ near-duplicate corrections) and lack of query-conditional logic are the named pain points — the recommended path is a **classifier (intent router) + small cross-encoder + MMR** pipeline. Anthropic's own Contextual Retrieval write-up reports a 67% reduction in retrieval failure rate from stacking BM25 + embeddings + reranker, suggesting this stack is well-supported by production data. The "simple can be enough" caveat is real: MMR (1998) and RRF (2009) are decades-old, ship in Qdrant/Pinecone today, and cost ~zero to add to a Rust backend.

## Current state in zbot

zbot today combines FTS5 + sqlite-vec via Reciprocal Rank Fusion, then applies a static post-fusion rescore chain: category-weight multipliers (schema 1.6, correction 1.5, strategy 1.4), a 0.7× contradiction penalty, temporal decay, a 1.3× ward-affinity boost, supersession dropping, a 0.3 min-score floor, and a sort-and-truncate to top-10. The pipeline is hand-tuned, query-independent, has no diversity step, and does not learn from feedback. Symptoms include near-duplicate top-K results and noise floods when memory grows (e.g., 30+ unrelated corrections rising on a single topic match).

## Approach catalog

### 1. Cross-encoder rerankers

**What it does.** A cross-encoder takes `(query, candidate)` as a *joint* input through a transformer, producing a single relevance score. Unlike bi-encoders (which embed query and document independently and compare), the cross-encoder lets every query token attend to every candidate token — much higher fidelity but O(N) inference at query time, not pre-computable [Pinecone, "Rerankers and Two-Stage Retrieval"; SBERT docs].

**Source / canonical implementations.** Original MS-MARCO MiniLM cross-encoders (Reimers/Gurevych, SBERT); `BAAI/bge-reranker-v2-m3` (Mar 2024, HF); `jinaai/jina-reranker-v2-base-multilingual` (Jun 2024); Cohere Rerank 3.5; FlashRank.

**Benchmarks (NDCG@10 across mixed retrieval benchmarks, from Rana 2025 "Top 8 Rerankers"):**

| Model | NDCG@10 | p95 Latency | Self-host cost / 1k q |
|---|---|---|---|
| MiniLM-L-6-v2 (msmarco) | 0.662 | 55 ms | ~$0.08 |
| bge-reranker-base v2 | 0.699 | 92 ms | ~$0.18 |
| Jina reranker v2 multilingual | 0.694 | 110 ms | ~$0.30 |
| bge-reranker-large v2 | 0.715 | 145 ms | ~$0.35 |
| Cohere Rerank 3.5 | 0.735 | 210 ms (API) | $2.40 (API) |
| MonoT5-3B | 0.726 | 480 ms | ~$1.25 |

BGE-reranker-v2-m3 achieves an **average NDCG@10 of 0.6585 across BEIR** in its own paper (FlagEmbedding). Cross-encoders typically add **+3 to +7 NDCG@10** over bi-encoder retrieval alone on BEIR.

**When it wins.** Mid-sized top-K (50–200 candidates), latency budget ≥ 100 ms, you have GPU or ONNX-on-CPU.
**When it loses.** Latency-critical (<50ms/query), or when query+doc length is large (BERT's 512-token limit becomes painful).
**Rust effort.** Low. `fastembed` (Anush008/fastembed-rs) exposes a `TextRerank` struct with `BGERerankerBase`, `MSMARCO-MiniLM-L-6/L-12` via ONNX. `candle` is the alternative for safetensors. Drop-in replacement for the static rescore chain.

### 2. LLM-as-judge / listwise reranking

**What it does.** Pass top-K candidates to an LLM as a numbered list; prompt it to emit the reordered list (e.g., `[3] > [1] > [7]`). Listwise because the LLM sees all candidates simultaneously and can reason about their relative merit, unlike pointwise scoring [Sun et al., "Is ChatGPT Good at Search?", EMNLP 2023, arXiv:2304.09542].

**Source.** RankGPT (Sun 2023, EMNLP Outstanding Paper); RankVicuna and RankZephyr (Castorini, open-source 7B distillations); ListT5 (ACL 2024); RankLLM toolkit.

**Benchmarks.** RankGPT (GPT-4) reaches **NDCG@10 = 0.7559 on TREC-DL19** and 0.8551 on TREC-COVID — current state-of-the-art among rerankers. GPT-4 averages **NDCG@10 ≈ 0.5368 across BEIR** and 0.6293 on Mr.TyDi multilingual. The distilled 440M RankGPT-distilled model beats a 3B supervised baseline on BEIR. RankZephyr 7B matches GPT-3.5 zero-shot performance. To handle >K passages within context limits, RankGPT uses a sliding window (window n, stride m, scanned back-to-front).

**Cost.** A single rerank with GPT-4o-class models on 20 passages is 1 LLM call (~1–4k tokens in, ~50 tokens out). At ~$5/1M input for GPT-4o, that is ~$0.005–$0.02/query — orders of magnitude more than a cross-encoder.
**When it wins.** Highest-quality offline workflows; corpora outside cross-encoder training distribution.
**When it loses.** Per-query cost matters; latency-critical paths; high QPS.
**Rust effort.** Low–medium. Already have LLM clients in zbot; just prompt-engineer + parse `[i] > [j]` output. The risk is the LLM being inconsistent (mitigated by "permutation self-consistency", Tang et al. 2024, arXiv:2310.07712).

### 3. Query classification → conditional retrieval (intent router)

**What it does.** Classify the incoming query (factoid? navigational? conversational? procedural?) then dispatch to a strategy: different retrievers, different weights, different rerankers, or different top-K. Production RAG calls this a "router" [LlamaIndex `RouterQueryEngine`, LangChain semantic router, Aurelio Semantic Router].

**Source / systems.**
- **Aurelio Semantic Router** — uses kNN over labelled utterance embeddings; ~100 ms vs ~5000 ms for LLM routing.
- **LlamaIndex `RouterQueryEngine` / `RouterRetriever`** — LLM-selector or Pydantic-selector picks one or many sub-engines.
- **RAGRouter** (Wang et al., arXiv:2505.23052, 2025) — learns to route to the best of several RAG-augmented LLMs via contrastive learning over capability embeddings.
- **DistilBERT intent classifier** (Rathod, Medium 2024): ~85% accuracy, <60 ms classification — referenced as 10× cheaper than GPT.

**Benchmarks.** No single canonical benchmark for "router quality"; existing studies report ~85% accuracy for small classifiers (DistilBERT, fastText) on intent datasets. The win is downstream: feeding the right query to the right retriever stack.
**When it wins.** Mixed query distributions (your case — factoid recall vs. conversational continuation vs. procedural). Big wins when one fixed pipeline either over-recalls noise or under-recalls signal.
**When it loses.** Homogeneous query distribution; misclassification cost > average gain. Class boundaries are also fuzzy in practice ("which router class is *'remind me what we decided about X yesterday'*?").
**Rust effort.** Low (embedding kNN over labelled examples) to medium (LLM-classifier with cache). The Aurelio approach maps cleanly: store a `Vec<(intent, embedding)>` in SQLite, embed the query, cosine-nearest. No new infra.

### 4. Maximal Marginal Relevance (MMR)

**What it does.** Greedy re-selection that, at each step, picks the next item to maximize `λ · sim(item, query) − (1−λ) · max sim(item, already_selected)`. λ ∈ [0,1] trades relevance for diversity. From Carbonell & Goldstein, SIGIR 1998 ("The Use of MMR, Diversity-Based Reranking…").

**Benchmarks.** No single canonical RAG number — but MMR is built into Qdrant natively and into Pinecone/Weaviate via LangChain. Empirically across recommendation literature, MMR improves user satisfaction by reducing top-K redundancy without large relevance losses when λ ∈ [0.5, 0.8].

**When it wins.** Top-K contains semantic near-duplicates (your "30 corrections about unrelated topic" symptom is partly this). Cheap O(K²) post-step.
**When it loses.** When all candidates *should* be similar (focused factoid recall). Over-diversification with λ too low can drop the actual best answer to position 3.
**Rust effort.** Trivial. ~30 lines of Rust over the existing top-K vectors. Already have cosine similarity.

### 5. Learning to Rank (LambdaMART / XGBoost)

**What it does.** Train a gradient-boosted decision tree (LambdaMART) on `(query, doc, features, relevance_label)` tuples to directly optimize NDCG. Features can be anything: BM25 score, vector cosine, recency, category, click feedback. [XGBoost docs `rank:ndcg`; Elastic LTR plugin.]

**Source.** Burges et al., LambdaRank/LambdaMART (Microsoft Research). XGBoost ships `rank:ndcg`, `rank:pairwise` objectives. NVIDIA blog on GPU-accelerated XGBoost-LTR.

**Benchmarks.** LambdaMART remains a strong tabular-feature baseline; specific numbers depend on feature set. The catch: you need **labelled relevance data**. Click-through is the standard surrogate but requires de-biasing (XGBoost includes Unbiased LambdaMART for this).

**When it wins.** Mature systems with labelled feedback, many sparse features (recency, category, ward, contradictions, supersession — exactly zbot's situation if you log "user kept this memory in context" signals).
**When it loses.** Cold start — no labels means no training signal. Static rule-tuning (zbot today) is often better than a bad LTR model.
**Rust effort.** Medium. No first-class Rust LambdaMART; typical pattern is offline training in Python (`xgboost`) and serving via the `xgboost` C library FFI or by serializing to ONNX. `lightgbm` Rust bindings exist but are thin.

### 6. Reciprocal Rank Fusion at the rerank layer

**What it does.** `score(d) = Σ 1/(k + rank_i(d))` across multiple ranked lists; canonical k=60. Originally introduced for fusing the outputs of multiple search systems (Cormack, Clarke, Büttcher, SIGIR 2009, "Reciprocal Rank Fusion outperforms Condorcet and individual Rank Learning Methods").

**Benchmarks.** Cormack's original paper beat CombMNZ, Condorcet Fuse, and every individual LETOR-3 learning-to-rank method tested. Plateau-flat over k ∈ [20, 100] for MAP. zbot already uses it at the FTS+vector layer.

**Beyond just FTS+vector fusion.** RRF can also blend **multiple rerankers' outputs** (e.g., BM25 + cross-encoder + LLM judge) — useful when you have heterogeneous score scales and don't trust any single ranker.
**Vs. weighted sum / z-score.** Use weighted sum when you have eval data to calibrate weights; use RRF when scores are non-comparable or you have no calibration data (typical for new systems).
**When it loses.** RRF discards score magnitudes — if one ranker is *much* more confident, that signal is thrown away.
**Rust effort.** Trivial (already have it).

### 7. Late interaction (ColBERT / ColBERTv2)

**What it does.** Bi-encoder at indexing time, but stores **per-token vectors** rather than one pooled vector. At query time, scores each query token against all doc tokens via a "MaxSim" operation. Bridges bi-encoder speed and cross-encoder fidelity. [Santhanam et al., NAACL 2022, arXiv:2112.01488.]

**Benchmarks.** ColBERTv2 sets state-of-the-art on BEIR among bi-encoder-style retrievers while compressing storage 6–10× via residual compression. With PLAID engine, latency is **tens of ms on GPU, low hundreds on CPU at 140M-passage scale** — and PLAID is 7×/45× faster than vanilla ColBERTv2 on GPU/CPU respectively.

**When it wins.** Very large corpora where you want cross-encoder-grade quality at bi-encoder-grade latency.
**When it loses.** Small corpora (zbot's memory is small); storage cost still 6–10× a normal embedding.
**Rust effort.** High. No mature Rust ColBERT implementation; `embed_anything` claims late-interaction support. Significant operational complexity for marginal value at zbot's scale.

### 8. Hybrid + rerank pipelines from production systems

**What it does.** Combine BM25 (or BM42) + dense vectors + reranker, sometimes plus a contextual augmentation step.

**Anthropic's Contextual Retrieval (Sep 2024)** is the canonical reference: prepend an LLM-generated 50-100-token context to each chunk *before* embedding and BM25-indexing. Cached via prompt caching at ~$1.02 per million doc tokens (one-time).
- Contextual Embeddings alone: **35% failure-rate reduction** (5.7% → 3.7%)
- + Contextual BM25: **49%** (→ 2.9%)
- + Reranking: **67%** (→ 1.9%)
- Test domains: codebases, fiction, ArXiv, science.

**Pinecone's "cascading retrieval"** (2024 blog) unifies dense + sparse + reranking, with hosted models including `cohere-rerank-3.5`, `bge-reranker-v2-m3`, `pinecone-rerank-v0`.

**LangChain & LlamaIndex** offer reranker integrations as drop-in `BaseRetriever` decorators.

**When it wins.** Almost always — adding a small reranker to a hybrid retriever is the single most-cited improvement in production RAG literature.
**Rust effort.** Low–medium. Layer-by-layer addition over existing FTS+vector+RRF.

### 9. Diversity-aware / Determinantal Point Processes (DPP)

**What it does.** A probabilistic model over *sets*: P(S) ∝ det(L_S) for a kernel matrix L. Encodes a tradeoff between item quality (diagonal) and pairwise similarity (off-diagonal) — "repulsion". Greedy MAP approximation is the standard practical algorithm. [Kulesza & Taskar 2012; Chen et al. NeurIPS 2018 "Fast Greedy MAP Inference for DPP"; Wilhelm et al. CIKM 2018 "Practical Diversified Recommendations on YouTube with DPP".]

**Benchmarks.** YouTube's blog/paper showed +0.6% engagement lift from DPP vs. baseline diversification. NeurIPS 2018 paper showed greedy MAP-DPP runs in <1 ms for top-100 sets when properly cached.
**When it wins.** Larger top-K (>20), when MMR's pairwise-max term is too local — DPP considers the *whole set* simultaneously.
**When it loses.** At zbot's top-10 scale, MMR is hard to beat and 10× simpler.
**Rust effort.** Medium. Requires Cholesky / determinant of small dense matrices — `nalgebra` is sufficient.

### 10. Simple but effective baselines

**Query expansion / HyDE.** Generate a hypothetical answer from the LLM, embed *that*, retrieve neighbours. Gao et al., 2022, arXiv:2212.10496. HyDE outperformed Contriever (unsupervised dense retriever) and approached fine-tuned-retriever performance with no labelled data. Costs: one extra LLM call per query.

**query2doc (Wang et al. 2023).** Same idea, but the LLM expansion is *prepended to the original query* rather than replacing it. Both query2doc and HyDE consistently outperformed BM25 and Contriever across seven LLM backbones in published evaluation.

**Pseudo-Relevance Feedback (RM3, Rocchio).** Run an initial query, take top-K docs, extract their high-IDF terms, re-issue an expanded query. RM3 remains a strong baseline; competitive with Query2Doc/MuGI on BEIR despite using no LLM (Lin et al., Anserini).

**BM25-only fallback.** For navigational/exact-match queries ("what's my OpenAI key path?"), BM25 alone often beats vector search.
**When they win.** Tight budgets, no GPU, fallback paths for cold-start.
**Rust effort.** Trivial — HyDE is one extra LLM call; RM3 is a few SQL queries away from your FTS index.

## Comparison matrix

| Approach | Quality (NDCG@10 lift over hybrid baseline) | Latency (per query) | Cost | Implementation effort (Rust) | When to use |
|---|---|---|---|---|---|
| MMR | ~0 (relevance ≈ flat, diversity ↑) | <1 ms | $0 | Trivial | Top-K has near-duplicates |
| RRF at rerank layer | +0–3 | <1 ms | $0 | Trivial | Blending multiple rankers, no eval data |
| HyDE / query2doc | +2–6 | +500 ms (extra LLM call) | $$ | Low | Out-of-domain queries, no labels |
| RM3 / Rocchio PRF | +1–3 | +20 ms (2nd FTS pass) | $0 | Low | Pure keyword fallback |
| Intent router (kNN-classifier) | depends; +2–8 indirect | <10 ms | $0 | Low | Mixed query distribution |
| MiniLM cross-encoder | +3–5 | 55 ms / 100 docs | $0 (ONNX) | Low (fastembed) | Default cross-encoder for desktop |
| BGE-reranker-v2-base | +4–6 | 92 ms / 100 docs | $0 (ONNX) | Low (fastembed) | Higher quality desktop reranker |
| BGE-reranker-v2-large | +5–7 | 145 ms / 100 docs | $$ (GPU helpful) | Low | Server-tier reranker |
| Cohere Rerank 3.5 | +6–8 | 150–400 ms (API) | $2.40/1k | Low (HTTP) | Hosted, no infra |
| ColBERTv2 + PLAID | +4–6 | 10–200 ms | Storage 6–10× | High | Large corpora |
| LambdaMART / XGBoost LTR | +3–10 if well-labelled | <5 ms | $0 | Medium (FFI) | You have feedback labels |
| RankGPT / RankZephyr | +5–10 (TREC-DL benchmarks) | 1–3 s | $$$$ | Low–medium | Best quality, offline OK |
| DPP (greedy MAP) | ~0 relevance, diversity ↑↑ | 1–10 ms | $0 | Medium | Top-K > 20 with diversity goal |
| Contextual Retrieval | 67% failure-rate reduction (Anthropic) | indexing-time | $1.02/MTok one-time | Medium (re-index) | Chunks lose context (zbot facts likely do) |

## Classifier + rerank specifically

The user's stated interest is the **classifier-routed rerank pipeline**. Here is what the literature and production systems converge on.

**Classify the query, not the candidate.** Across LlamaIndex, LangChain, Aurelio, and RAGRouter, the dominant pattern is *query classification* — the candidates remain a function of the chosen retriever/reranker route. Per-candidate classification (e.g., "is this fact factual or conversational?") shows up in different forms — content tagging, schema typing — but the routing decision itself is made on the query side, because the query is what determines *what kind of answer is needed*. zbot's category-weight multipliers (schema 1.6, correction 1.5, etc.) are already a static form of per-candidate signal; what's missing is query-conditional reweighting.

**Three router implementation tiers, from cheapest to most expressive:**

1. **kNN-over-utterances (Aurelio Semantic Router pattern).** Author a short labelled list — say 10–30 example queries per intent class — embed them with your existing embedding model, store. At query time: embed the query, cosine-nearest, vote among k=5 neighbours, pick the intent. ~100 ms end-to-end versus ~5000 ms for an LLM classifier per Aurelio. This is the lowest-friction option for zbot because the embedding infrastructure already exists.

2. **DistilBERT or fastText classifier.** Train a small classifier on a few hundred labelled examples. Reported numbers: ~85% accuracy, <60 ms per classification (Rathod 2024). Requires labelled data, model file, ONNX inference. More capable than kNN at the cost of an offline training pipeline.

3. **LLM classifier (LlamaIndex `LLMSingleSelector`, LangChain `RouterChain`).** Prompt the LLM with "Given the query, choose one of: factoid | conversational | procedural | navigational | reflective. Output one word." Most flexible, slowest (200–2000 ms depending on LLM), highest cost. LLM routers are state-of-the-art when prompted well, per Patronus AI's routing tutorial, but zbot already pays the latency cost of a planning LLM call on most paths — routing could piggyback on the existing call rather than adding a new one.

**Conditional retrieval recipes commonly seen in production:**

- *Factoid* (e.g., "what's my OpenAI key path?") → BM25-heavy weighting + low diversity (MMR λ → 1.0) + small top-K + no cross-encoder (BM25 score is the signal).
- *Conversational* (e.g., "remind me what we were doing last session") → vector-heavy + temporal decay relaxed + cross-encoder + MMR moderate (λ ≈ 0.7).
- *Procedural* (e.g., "how did I usually run the daemon") → category-weight on `strategy` boosted + dedup heavy (MMR λ ≈ 0.6) + cross-encoder.
- *Reflective* (e.g., "what mistakes have I been making lately") → category-weight on `correction` boosted + LLM listwise rerank for ordering significance + larger top-K.

**Anthropic's Contextual Retrieval as the pipeline benchmark.** Anthropic's published numbers — 67% reduction in retrieval failure rate from stacking BM25 + embeddings + reranker — are the closest thing the field has to a vendor-neutral target for what a hybrid + rerank pipeline can achieve. Their pipeline *does not* use a classifier; the gain comes from prepending context to chunks and adding a single reranker. This is important to internalize: even *without* a router, just adding a reranker to a hybrid retriever is the single most-cited improvement in production RAG. The classifier is a multiplier on top.

**Cohere's two-stage retrieval (retrieve → rerank with confidence weighting).** Cohere documents a pattern where the rerank score itself serves as a confidence signal — below a threshold, the system can fall back to "I don't know" or escalate to a heavier retriever. zbot already has a min-score filter (0.3); replacing it with a cross-encoder-confidence-derived threshold would be a tiny code change with disproportionate benefit on noise queries.

**Recommended composition for zbot.** A minimal but research-backed classifier+rerank pipeline looks like:

```
query → semantic-router (kNN, ~50ms) → {intent class}
     → (intent-specific) hybrid retrieve (FTS + vec, RRF, top-50)
     → (intent-specific) cross-encoder rerank (MiniLM or BGE-base, ~90ms)
     → MMR (λ from intent, ~1ms)
     → confidence threshold drop
     → top-10
```

This adds ~150 ms of latency to today's pipeline and removes the static category-weight chain. Citations supporting each component: kNN router (Aurelio Labs); cross-encoder rerank (Pinecone, Anthropic, Cohere); MMR (Carbonell & Goldstein 1998); confidence threshold (Cohere docs); intent-conditional weights (LlamaIndex `RouterRetriever`).

## Recommended path for zbot

### Tier 1 — Do soon (high ROI, low effort)

1. **Add MMR to the existing top-K.** Sets λ = 0.7 by default; tunable. Directly addresses the "near-duplicate corrections" symptom. ~50 lines of Rust over the current rescore step. Carbonell & Goldstein 1998 is the canonical paper; Qdrant ships it natively as a reference implementation pattern.
2. **Swap the static rescore for a small cross-encoder.** `fastembed-rs` exposes `BGERerankerBase` and `MS-MARCO-MiniLM` via ONNX with no Tokio dependency. ~90 ms / 100 candidates on CPU per published benchmarks. Replaces 4 of the 8 static heuristic steps with a learned model trained on millions of MS-MARCO pairs.
3. **Add a kNN intent router.** Author 5–10 labelled examples for each of {factoid, conversational, procedural, reflective}, embed with the existing embedding model, store in SQLite. At query time, cosine-nearest → intent → pick a small intent-specific config (top-K, MMR λ, reranker on/off). Aurelio Semantic Router is the model; zbot has all the infrastructure.

Combined, Tier 1 is ~1–2 weeks of work, removes most of the static-heuristic surface area, and tracks Anthropic's published 67% failure-rate-reduction recipe (BM25 + embeddings + reranker), plus the diversity step.

### Tier 2 — Do later if Tier 1 isn't enough

4. **Contextual chunks at indexing time.** Per Anthropic, prepending 50–100 tokens of LLM-generated context to each fact before embedding/FTS-indexing is a one-time cost (~$1/1M tokens) and contributed roughly half of their headline gain. zbot's facts (especially corrections and schema items) likely lose context the same way Anthropic's chunked documents do.
5. **Confidence-thresholded reranker output.** Replace the static 0.3 floor with a percentile-based threshold derived from cross-encoder scores. Drop or escalate below the threshold (Cohere pattern).
6. **HyDE for reflective queries only.** For the specific class of "what was I doing / what mistakes have I made" queries, one extra LLM hop to generate a hypothetical answer and re-embed is empirically the cheapest way to bridge the query/answer language gap (Gao et al. 2022).

### Tier 3 — Research-level, defer

7. **Listwise LLM reranking (RankGPT-style).** Reserve for one-off batch tasks (e.g., "abstract a week of corrections into facts" — basically what the new `CorrectionsAbstractor` does). Not for online recall.
8. **LambdaMART/XGBoost LTR.** Only after you have logged feedback labels ("user kept this memory in context" / "user re-asked, so the recall was wrong"). Cold-start without labels is worse than tuned heuristics.
9. **DPP.** Only if top-K grows past 20 and MMR's local-pair max term proves insufficient. At top-10, MMR is competitive and 10× simpler.
10. **ColBERTv2.** Defer indefinitely — zbot's corpus is small, late-interaction's storage cost is real, and Rust support is immature.

## References

- Anthropic. "Introducing Contextual Retrieval." Anthropic Engineering Blog, 19 Sep 2024. https://www.anthropic.com/news/contextual-retrieval
- Carbonell, J. and Goldstein, J. "The Use of MMR, Diversity-Based Reranking for Reordering Documents and Producing Summaries." SIGIR 1998. https://www.cs.cmu.edu/~jgc/publication/The_Use_MMR_Diversity_Based_LTMIR_1998.pdf
- Chen, L., Zhang, G., and Zhou, H. "Fast Greedy MAP Inference for Determinantal Point Process to Improve Recommendation Diversity." NeurIPS 2018. arXiv:1709.05135.
- Cormack, G., Clarke, C., and Büttcher, S. "Reciprocal Rank Fusion Outperforms Condorcet and Individual Rank Learning Methods." SIGIR 2009. https://cormack.uwaterloo.ca/cormacksigir09-rrf.pdf
- Gao, L., Ma, X., Lin, J., and Callan, J. "Precise Zero-Shot Dense Retrieval without Relevance Labels" (HyDE). 2022. arXiv:2212.10496.
- Khattab, O. and Zaharia, M. "ColBERT: Efficient and Effective Passage Search via Contextualized Late Interaction over BERT." SIGIR 2020.
- Santhanam, K., Khattab, O., Saad-Falcon, J., Potts, C., and Zaharia, M. "ColBERTv2: Effective and Efficient Retrieval via Lightweight Late Interaction." NAACL 2022. arXiv:2112.01488.
- Sun, W. et al. "Is ChatGPT Good at Search? Investigating Large Language Models as Re-Ranking Agents" (RankGPT). EMNLP 2023 Outstanding Paper. arXiv:2304.09542.
- Pradeep, R., Sharifymoghaddam, S., and Lin, J. "RankZephyr: Effective and Robust Zero-Shot Listwise Reranking is a Breeze!" arXiv:2312.02724, 2023.
- Pradeep, R., Sharifymoghaddam, S., and Lin, J. "RankVicuna: Zero-Shot Listwise Document Reranking with Open-Source Large Language Models." 2023.
- Tang, R. et al. "Found in the Middle: Permutation Self-Consistency Improves Listwise Ranking in Large Language Models." arXiv:2310.07712.
- Wang, L. et al. "Query2doc: Query Expansion with Large Language Models." 2023.
- Wang, L. et al. "RAGRouter: Learning to Route Queries to Multiple Retrieval-Augmented Language Models." 2025. arXiv:2505.23052.
- Wilhelm, M. et al. "Practical Diversified Recommendations on YouTube with Determinantal Point Processes." CIKM 2018. https://jgillenw.com/cikm2018.pdf
- Kulesza, A. and Taskar, B. "Determinantal Point Processes for Machine Learning." Foundations and Trends in ML, 2012.
- BAAI. "bge-reranker-v2-m3." Hugging Face, 2024. https://huggingface.co/BAAI/bge-reranker-v2-m3
- Jina AI. "jina-reranker-v2-base-multilingual." 25 Jun 2024. https://huggingface.co/jinaai/jina-reranker-v2-base-multilingual
- Cohere. "Rerank 3.5." Product documentation, 2024–2025. https://docs.cohere.com/docs/rerank
- Reimers, N. and Gurevych, I. "Sentence-BERT: Sentence Embeddings using Siamese BERT-Networks." EMNLP 2019. (Bi-encoder / cross-encoder foundations.) https://sbert.net/
- Pinecone. "Rerankers and Two-Stage Retrieval." Learn series. https://www.pinecone.io/learn/series/rag/rerankers/
- Pinecone. "Introducing cascading retrieval: Unifying dense and sparse with reranking." Blog, 2024. https://www.pinecone.io/blog/cascading-retrieval/
- LlamaIndex. "Routers" module documentation. https://developers.llamaindex.ai/python/framework/module_guides/querying/router/
- Aurelio Labs. "Semantic Router." https://github.com/aurelio-labs/semantic-router and https://www.aurelio.ai/semantic-router
- Rana, B. "Top 8 Rerankers: Quality vs Cost — A practical benchmark of modern rerankers." Medium, 2025. https://medium.com/@bhagyarana80/top-8-rerankers-quality-vs-cost-4e9e63b73de8
- Rathod, D. "Intent Classification in <1ms: How We Built a Lightning-Fast Classifier with Embeddings." Medium, 2024.
- Anush008. `fastembed-rs` — Rust library for vector embeddings and reranking. https://github.com/Anush008/fastembed-rs
- Castorini. `rank_llm` — Python toolkit for LLM-based reranking research (RankGPT, RankVicuna, RankZephyr). https://github.com/castorini/rank_llm
- XGBoost developers. "Learning to Rank" tutorial. https://xgboost.readthedocs.io/en/stable/tutorials/learning_to_rank.html
- Elastic. "Elasticsearch Learning to Rank (LTR): Improving search ranking." Elastic Search Labs.
- "Enhancing Q&A Text Retrieval with Ranking Models: Benchmarking, fine-tuning and deploying Rerankers for RAG." arXiv:2409.07691.
