"""Reusable local media analysis. Model downloads require explicit opt-in."""
from ._native import (
    __version__, compare_maps, Analyzer, Index, SaccadeError, InputError, ModelError, AnalysisError,
    pull_models, pull_runtime,
)
__all__ = ["__version__", "compare_maps", "Analyzer", "Index", "SaccadeError", "InputError", "ModelError",
           "AnalysisError", "pull_models", "pull_runtime"]
