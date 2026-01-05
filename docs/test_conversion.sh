#!/bin/bash

# Test script for test.mid conversion
# This script validates the conversion process

echo "=================================="
echo "  MIDI to MML Conversion Test"
echo "=================================="
echo ""

# Check if test.mid exists
if [ ! -f "test.mid" ]; then
    echo "❌ Error: test.mid not found in current directory"
    exit 1
fi

echo "✓ Found test.mid"
echo ""

# Run analyzer
echo "Running MIDI analyzer..."
echo "=================================="
cd src-tauri
cargo build --bin analyze --quiet 2>&1 > /dev/null
if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

./target/debug/analyze ../test.mid > ../test_analysis.txt 2>&1

if [ $? -eq 0 ]; then
    echo "✓ Analysis completed successfully"
    echo ""
    
    # Extract key metrics
    echo "Key Metrics:"
    echo "=================================="
    grep "Extracted.*notes" ../test_analysis.txt
    grep "BPM:" ../test_analysis.txt | head -1
    grep "Allocated.*voices" ../test_analysis.txt
    
    echo ""
    echo "Voice MML Lengths:"
    grep "Voice.*chars" ../test_analysis.txt
    
    echo ""
    echo "First Chord Analysis:"
    grep -A 5 "Tick 576" ../test_analysis.txt | head -6
    
    echo ""
    echo "=================================="
    echo "✓ All tests passed!"
    echo "Full report saved to: test_analysis.txt"
else
    echo "❌ Analysis failed"
    exit 1
fi

cd ..