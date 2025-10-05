#!/usr/bin/env python3
import json
import sys

errors = []
warnings = []

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        data = json.loads(line)
        if data.get('reason') == 'compiler-message':
            msg = data.get('message', {})
            level = msg.get('level')
            if level in ('error', 'warning'):
                spans = msg.get('spans', [])
                if spans:
                    span = spans[0]
                    file_name = span.get('file_name', 'unknown')
                    line_start = span.get('line_start', 0)
                    message = msg.get('message', '')
                    
                    entry = f"{level}|{file_name}:{line_start}|{message}"
                    
                    if level == 'error':
                        errors.append(entry)
                    else:
                        warnings.append(entry)
    except json.JSONDecodeError:
        pass

# Print unique entries
print("=== ERRORS ===")
for e in sorted(set(errors)):
    print(e)

print("\n=== WARNINGS ===")
for w in sorted(set(warnings)):
    print(w)
