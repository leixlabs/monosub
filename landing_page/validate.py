#!/usr/bin/env python3
"""
Landing Page Integrity Validation Script
Checks for broken media paths and HTML structure issues
"""

import os
import re
from pathlib import Path

def validate_landing_page(html_path, base_dir):
    """Validate the landing page for common issues"""
    
    print("🔍 MonoSub Landing Page Validation\n")
    print("=" * 50)
    
    # Read HTML content
    with open(html_path, 'r', encoding='utf-8') as f:
        html_content = f.read()
    
    issues = []
    warnings = []
    
    # 1. Check for image references
    print("\n📸 Checking image references...")
    img_pattern = r'<img[^>]+src=["\'](\.\/imgs\/[^"\']+)["\']'
    img_matches = re.findall(img_pattern, html_content)
    
    for img_src in img_matches:
        # Remove ./ prefix
        clean_path = img_src.replace('./', '')
        full_path = base_dir / clean_path
        
        if full_path.exists():
            file_size = full_path.stat().st_size
            print(f"  ✅ {img_src} → EXISTS ({file_size:,} bytes)")
        else:
            issues.append(f"Missing image: {img_src}")
            print(f"  ❌ {img_src} → NOT FOUND")
    
    # 2. Check for video references (if any)
    print("\n🎬 Checking video references...")
    video_pattern = r'<(?:video|source)[^>]+src=["\'](\.\/videos\/[^"\']+)["\']'
    video_matches = re.findall(video_pattern, html_content)
    
    if video_matches:
        for video_src in video_matches:
            clean_path = video_src.replace('./', '')
            full_path = base_dir / clean_path
            
            if full_path.exists():
                file_size = full_path.stat().st_size
                print(f"  ✅ {video_src} → EXISTS ({file_size:,} bytes)")
            else:
                issues.append(f"Missing video: {video_src}")
                print(f"  ❌ {video_src} → NOT FOUND")
    else:
        print("  ℹ️  No video references found")
    
    # 3. Check for CDN resources
    print("\n🌐 Checking external CDN resources...")
    cdn_pattern = r'(?:href|src)=["\'](https?://[^"\']+)["\']'
    cdn_matches = set(re.findall(cdn_pattern, html_content))
    
    for cdn_url in sorted(cdn_matches):
        if 'tailwindcss' in cdn_url:
            print(f"  ✅ Tailwind CSS: {cdn_url}")
        elif 'googleapis' in cdn_url or 'gstatic' in cdn_url:
            print(f"  ✅ Google Fonts: {cdn_url}")
        else:
            print(f"  ℹ️  External: {cdn_url}")
    
    # 4. Check for critical sections
    print("\n📄 Checking page structure...")
    critical_sections = [
        ('Hero Section', r'<section[^>]*class="[^"]*min-h-screen[^"]*"'),
        ('Navigation', r'<nav[^>]*id="navbar"'),
        ('Features Section', r'id="features"'),
        ('Pricing Section', r'id="pricing"'),
        ('FAQ Section', r'id="faq"'),
        ('CTA Section', r'id="cta"'),
        ('Footer', r'<footer'),
    ]
    
    for section_name, pattern in critical_sections:
        if re.search(pattern, html_content):
            print(f"  ✅ {section_name}")
        else:
            warnings.append(f"Missing section: {section_name}")
            print(f"  ⚠️  {section_name}")
    
    # 5. Check for JavaScript functionality
    print("\n⚙️ Checking JavaScript...")
    js_checks = [
        ('Scroll animations', r'IntersectionObserver'),
        ('FAQ toggle', r'toggleFAQ'),
        ('Mobile menu', r'mobile-menu'),
        ('Smooth scroll', r'scroll-smooth|scrollIntoView'),
    ]
    
    for check_name, pattern in js_checks:
        if re.search(pattern, html_content, re.IGNORECASE):
            print(f"  ✅ {check_name}")
        else:
            warnings.append(f"Missing JS: {check_name}")
            print(f"  ⚠️  {check_name}")
    
    # 6. Color palette compliance (NO BLUE/PURPLE)
    print("\n🎨 Checking color compliance (NO BLUE/PURPLE)...")
    
    # Check for forbidden colors
    forbidden_patterns = [
        (r'#[0-9a-fA-F]{0,2}[0-9a-fA-F]{2}[0-9a-fA-F]{2}ff', 'Possible blue color'),
        (r'bg-blue-', 'Tailwind blue class'),
        (r'text-blue-', 'Tailwind blue text'),
        (r'bg-purple-', 'Tailwind purple class'),
        (r'text-purple-', 'Tailwind purple text'),
        (r'bg-indigo-', 'Tailwind indigo class'),
        (r'text-indigo-', 'Tailwind indigo text'),
    ]
    
    color_violations = []
    for pattern, description in forbidden_patterns:
        matches = re.findall(pattern, html_content)
        if matches:
            color_violations.extend([(m, description) for m in matches])
    
    if color_violations:
        print(f"  ⚠️  Found {len(color_violations)} potential blue/purple violations")
        for violation, desc in color_violations[:5]:  # Show first 5
            print(f"      - {violation}: {desc}")
        issues.append(f"Color violations: {len(color_violations)} found")
    else:
        print("  ✅ No blue/purple colors detected")
    
    # Check for approved colors
    approved_colors = [
        (r'#f59e0b', 'Amber (primary CTA)'),
        (r'#ef4444', 'Red (gradient accent)'),
        (r'#22c55e', 'Green (success)'),
        (r'#0a0a0f', 'Background dark'),
        (r'amber-500', 'Tailwind amber'),
        (r'green-500', 'Tailwind green'),
    ]
    
    print("\n  Approved colors found:")
    for color, description in approved_colors:
        if re.search(color, html_content):
            print(f"    ✅ {description}")
    
    # Final Report
    print("\n" + "=" * 50)
    print("📊 VALIDATION SUMMARY\n")
    
    if not issues:
        print("✅ All critical checks passed!")
    else:
        print(f"❌ Found {len(issues)} issue(s):")
        for issue in issues:
            print(f"   - {issue}")
    
    if warnings:
        print(f"\n⚠️  {len(warnings)} warning(s):")
        for warning in warnings:
            print(f"   - {warning}")
    
    print("\n" + "=" * 50)
    
    # File size report
    html_size = os.path.getsize(html_path)
    print(f"\n📦 File Size: {html_size:,} bytes ({html_size/1024:.1f} KB)")
    
    return len(issues) == 0

if __name__ == "__main__":
    base_dir = Path("/Users/lei/dev/personal/monosub/landing_page")
    html_path = base_dir / "index.html"
    
    success = validate_landing_page(html_path, base_dir)
    
    if success:
        print("\n✅ Ready for deployment!")
        exit(0)
    else:
        print("\n❌ Fix issues before deployment")
        exit(1)
