"""Reusable local media analysis. Model downloads require explicit opt-in."""
from ._native import (
    Analyzer, Index, SaccadeError, InputError, ModelError, AnalysisError,
    pull_models, pull_runtime,
)
__all__ = ["Analyzer", "Index", "SaccadeError", "InputError", "ModelError",
           "AnalysisError", "pull_models", "pull_runtime"]
