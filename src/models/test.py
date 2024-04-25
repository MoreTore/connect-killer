import re

# Define the regex pattern
pattern = r"^([a-z0-9]{16})\|([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)"

# Compile the regex pattern
regex = re.compile(pattern)

# Example canonical name
canonical_name = "1234567890abcdef|2024-03-02--19-02-46--42"

# Search for matches
match = regex.match(canonical_name)
if match:
    print("Match found:")
    for i in range(1, 5):
        print(f"Group {i}: {match.group(i)}")
else:
    print("No match found")