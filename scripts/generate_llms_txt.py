#!/usr/bin/env python3
import os

OUTPUT_FILE = "llms.txt"
extensions = [".rs", ".md", ".toml", ".py", ".sh"]
ignore_dirs = ["target", ".git", ".github"]
ignore_files = ["Cargo.lock", "llms.txt"]

def generate_llms_txt():
    with open(OUTPUT_FILE, "w", encoding="utf-8") as outfile:
        # Walk through the directory
        for root, dirs, files in os.walk("."):
            # Modify dirs in-place to skip ignored directories
            dirs[:] = [d for d in dirs if d not in ignore_dirs]

            # Sort for deterministic output
            dirs.sort()
            files.sort()

            for file in files:
                if file in ignore_files:
                    continue

                _, ext = os.path.splitext(file)
                if ext in extensions or file in ["Dockerfile", "Makefile"]: # Add other exact matches if needed
                    file_path = os.path.join(root, file)

                    # Normalize path to use forward slashes and remove leading ./
                    rel_path = os.path.relpath(file_path, ".")

                    outfile.write(f"File: {rel_path}\n")
                    outfile.write("```\n")

                    try:
                        with open(file_path, "r", encoding="utf-8") as infile:
                            outfile.write(infile.read())
                    except Exception as e:
                        outfile.write(f"Error reading file: {e}\n")

                    outfile.write("\n```\n\n")

if __name__ == "__main__":
    generate_llms_txt()
    print(f"Generated {OUTPUT_FILE}")
