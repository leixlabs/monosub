"""
PyInstaller runtime hook to ensure FFmpeg is available in PATH
This runs when the bundled executable starts, before any imports
"""
import os
import sys

# When running as PyInstaller bundle, add FFmpeg to PATH
if getattr(sys, 'frozen', False):
    # CRITICAL FIX #1: Pre-load torch to avoid circular import issues
    # This must happen BEFORE any other imports that might trigger torch loading
    # Resolves: "cannot import name 'nested' from partially initialized module 'torch'"
    # See: https://github.com/pyinstaller/pyinstaller/issues/7697
    print("DEBUG: Pre-loading torch to avoid circular imports...", flush=True)
    try:
        import torch
        # Trigger full initialization to avoid partial initialization issues
        _ = torch.tensor([1.0])
        print("DEBUG: torch pre-loaded successfully", flush=True)
    except Exception as e:
        print(f"WARNING: Failed to pre-load torch: {e}", flush=True)

    # We're running as bundled executable
    bundle_dir = sys._MEIPASS

    # Check if ffmpeg/ffprobe exist in environment variables (set by Tauri)
    ffmpeg_path = os.environ.get('MONOSUB_FFMPEG_PATH', '')
    ffprobe_path = os.environ.get('MONOSUB_FFPROBE_PATH', '')

    if ffmpeg_path and ffprobe_path:
        # Extract directory from the full path
        ffmpeg_dir = os.path.dirname(ffmpeg_path)

        # Add to PATH so subprocess calls can find ffmpeg/ffprobe
        if ffmpeg_dir and os.path.exists(ffmpeg_dir):
            current_path = os.environ.get('PATH', '')
            if ffmpeg_dir not in current_path:
                os.environ['PATH'] = ffmpeg_dir + os.pathsep + current_path
                print(f"DEBUG: Added FFmpeg directory to PATH: {ffmpeg_dir}", flush=True)

        # CRITICAL FIX: Monkey patch subprocess.run to replace bare ffmpeg/ffprobe commands
        # This is necessary because FunASR internally calls 'ffmpeg' directly without
        # checking environment variables (funasr/utils/load_utils.py:266)
        import subprocess

        _original_run = subprocess.run

        def patched_run(args, *pargs, **kwargs):
            """
            Intercept subprocess.run calls and replace bare ffmpeg/ffprobe commands
            with full paths from environment variables
            """
            if isinstance(args, (list, tuple)) and len(args) > 0:
                # Replace 'ffmpeg' with full path
                if args[0] == 'ffmpeg' and ffmpeg_path:
                    args = list(args)  # Convert to list if tuple
                    args[0] = ffmpeg_path
                    print(f"DEBUG: Patched ffmpeg command to: {ffmpeg_path}", flush=True)
                # Replace 'ffprobe' with full path
                elif args[0] == 'ffprobe' and ffprobe_path:
                    args = list(args)  # Convert to list if tuple
                    args[0] = ffprobe_path
                    print(f"DEBUG: Patched ffprobe command to: {ffprobe_path}", flush=True)

            return _original_run(args, *pargs, **kwargs)

        # Replace subprocess.run globally
        subprocess.run = patched_run
        print("DEBUG: subprocess.run patched for FFmpeg/FFprobe", flush=True)
