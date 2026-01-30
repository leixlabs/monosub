"""
PyInstaller hook for torchaudio
Ensures torchaudio dynamic libraries are properly bundled
"""
from PyInstaller.utils.hooks import collect_dynamic_libs, collect_data_files, collect_submodules

# Collect all dynamic libraries
binaries = collect_dynamic_libs('torchaudio')

# Collect data files
datas = collect_data_files('torchaudio')

# Collect all submodules
hiddenimports = collect_submodules('torchaudio')

# Add critical imports
hiddenimports.extend([
    'torchaudio.backend',
    'torchaudio.transforms',
    'torchaudio.functional',
])
