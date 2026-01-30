"""
PyInstaller hook for PyTorch
Ensures all PyTorch dynamic libraries and dependencies are properly bundled
"""
from PyInstaller.utils.hooks import collect_dynamic_libs, collect_data_files, collect_submodules
import os
import sys

# Collect all dynamic libraries from torch
datas = []
binaries = []
hiddenimports = []

# Get torch installation path
try:
    import torch
    torch_dir = os.path.dirname(torch.__file__)

    # Collect all .dylib files (macOS) or .so files (Linux) or .dll files (Windows)
    torch_libs = collect_dynamic_libs('torch')
    binaries.extend(torch_libs)

    # Collect torch/lib directory (contains all dynamic libraries)
    lib_dir = os.path.join(torch_dir, 'lib')
    if os.path.exists(lib_dir):
        for root, dirs, files in os.walk(lib_dir):
            for file in files:
                if file.endswith(('.dylib', '.so', '.dll')):
                    src_path = os.path.join(root, file)
                    # Preserve directory structure relative to torch package
                    rel_path = os.path.relpath(root, torch_dir)
                    dst_dir = os.path.join('torch', rel_path)
                    binaries.append((src_path, dst_dir))

    # Collect data files (important for model configs)
    datas.extend(collect_data_files('torch'))

    # Collect all submodules
    hiddenimports.extend(collect_submodules('torch'))

    # Explicitly include critical submodules
    hiddenimports.extend([
        'torch._C',
        'torch._dl',
        'torch._appdirs',
        'torch.backends.mps',
        'torch.backends.cuda',
        'torch.nn',
        'torch.nn.functional',
        'torch.optim',
        'torch.autograd',
        'torch.utils.data',
    ])

except ImportError:
    print("WARNING: torch not found during PyInstaller hook execution")
