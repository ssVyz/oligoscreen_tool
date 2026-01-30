# Oligoscreen Tool

A Rust GUI application for screening aligned DNA sequences to identify optimal primer sites based on sequence variability.

## Purpose

Analyzes aligned FASTA sequences to determine how many oligonucleotide variants are required to achieve a specified coverage threshold at each position. Identifies conserved regions suitable for universal primer design.

## Requirements

- Rust 2021 edition
- Cargo

## Building

```bash
cargo build --release
```

## Usage

```bash
cargo run --release
```

### Workflow

1. **Input**: Load a FASTA file or paste aligned sequences
2. **Configure**: Set analysis parameters (oligo length range, coverage threshold, analysis method)
3. **Run**: Execute analysis
4. **Review**: Examine results via color-coded visualization; click positions for variant details
5. **Save**: Export results as JSON

### Input Requirements

- Standard FASTA format with aligned sequences
- Gaps represented as `-` or `.`
- Ambiguous IUPAC codes supported

### Parameters

| Parameter | Description |
|-----------|-------------|
| Analysis mode | Screen Alignment (sliding windows) or Single Oligo Region |
| Oligo length | Range of primer lengths to analyze (3-100 bp) |
| Resolution | Bases between analysis windows |
| Coverage threshold | Target percentage of sequences to cover (1-100%) |
| Analysis method | No Ambiguities, Fixed Ambiguities, or Incremental |
| Threads | Auto (all cores) or manual selection (1-N) |

## How It Works

### Analysis Modes

**Screen Alignment**: Default mode. Scans the alignment using sliding windows of specified lengths, analyzing variability at each position.

**Single Oligo Region**: For pre-aligned single oligo regions. Treats the entire alignment as one analysis window. Sequences containing gaps or ambiguous bases are excluded from analysis.

### Analysis Methods

**No Ambiguities**: Counts exact unique sequence variants at each window position.

**Fixed Ambiguities**: Uses greedy set cover to find minimum variants, allowing up to N IUPAC ambiguity codes per variant.

**Incremental**: Iteratively builds consensus sequences, each covering a percentage of remaining sequences until threshold is reached. Optional "Limit ambiguities" setting caps the maximum ambiguities per variant; if the target percentage cannot be reached within the limit, accepts the best variant within the constraint and continues.

### Window Filtering

Windows are skipped when:
- >20% of sequences contain gaps at that position
- >20% of sequences contain ambiguous bases

### Output

For each position and oligo length:
- Number of variants needed for coverage threshold
- Individual variant sequences with counts and percentages
- Color-coded visualization (green=1 variant, yellow=2-5, orange=6-10, red=>10, gray=skipped)

### Variant Detail Display Options

The position details window includes display options:
- **Reverse complement**: Show sequences as reverse complement (handles IUPAC codes)
- **Codon spacing**: Insert space every 3 bases (e.g., `ATA CCA TTC`) - enabled by default

## Dependencies

- egui/eframe - GUI framework
- serde/serde_json - Serialization
- rfd - Native file dialogs
- rayon - Parallelization

## License

TBD
