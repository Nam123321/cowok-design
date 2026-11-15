#!/usr/bin/env python3
import yaml
import argparse
import sys
from pathlib import Path

def load_library(filepath="docs/R&D-Library/R&D-Library.yaml"):
    try:
        with open(filepath, 'r') as f:
            return yaml.safe_load(f)
    except FileNotFoundError:
        print(f"Error: {filepath} not found.")
        sys.exit(1)

def query_library(data, target_tag=None, target_name=None):
    results = []
    
    categories = ['internal_assets', 'plugins']
    for category in categories:
        assets = data.get(category)
        if not assets:
            continue
            
        for asset in assets:
            match = False
            if target_tag and target_tag in asset.get('tags', []):
                match = True
            if target_name and target_name.lower() in asset.get('name', '').lower():
                match = True
            
            if match:
                asset['category'] = category
                results.append(asset)
                
    return results

def main():
    parser = argparse.ArgumentParser(description="Query the Cowok.ai R&D Library")
    parser.add_argument("--tag", help="Search by capability tag (e.g., 'document-parsing')")
    parser.add_argument("--name", help="Search by asset name (e.g., 'unstructured')")
    parser.add_argument("--file", default="docs/R&D-Library/R&D-Library.yaml", help="Path to R&D-Library.yaml")
    
    args = parser.parse_args()
    
    if not args.tag and not args.name:
        print("Please provide --tag or --name to search.")
        sys.exit(1)
        
    data = load_library(args.file)
    results = query_library(data, target_tag=args.tag, target_name=args.name)
    
    if not results:
        print("No matching assets found.")
        sys.exit(0)
        
    print(f"Found {len(results)} matching asset(s):\n")
    for r in results:
        print(f"- Name: {r.get('name')} (ID: {r.get('id')})")
        print(f"  Category: {r.get('category')}")
        print(f"  Tags: {', '.join(r.get('tags', []))}")
        print(f"  Status: {r.get('status')}")
        print(f"  Path: {r.get('path')}")
        print(f"  Limit Rate: {r.get('limit_rate')}")
        print("")

if __name__ == "__main__":
    main()
