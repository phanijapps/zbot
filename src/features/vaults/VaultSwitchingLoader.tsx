// ============================================================================
// VAULT SWITCHING LOADER
// Professional loading animation during vault transitions
// ============================================================================

interface VaultSwitchingLoaderProps {
  show: boolean;
}

export function VaultSwitchingLoader({ show }: VaultSwitchingLoaderProps) {
  if (!show) return null;

  return (
    <div className="fixed inset-0 z-[9999] flex items-center justify-center bg-[#0d0d0d]">
      <div className="text-center">
        {/* Animated Logo/Icon */}
        <div className="relative mb-8">
          {/* Outer ring */}
          <div className="absolute inset-0 animate-ping">
            <div className="w-24 h-24 rounded-full border-2 border-violet-500/30" />
          </div>
          {/* Middle spinning ring */}
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-20 h-20 rounded-full border-2 border-transparent border-t-violet-500 animate-spin" />
          </div>
          {/* Inner pulsing circle */}
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-12 h-12 rounded-full bg-gradient-to-br from-violet-500 to-purple-600 animate-pulse shadow-lg shadow-violet-500/50" />
          </div>
        </div>

        {/* Loading text with animation */}
        <div className="space-y-3">
          <h2 className="text-xl font-semibold text-white">Switching Vaults</h2>
          <p className="text-gray-400 text-sm">Loading your workspace...</p>
        </div>

        {/* Progress dots */}
        <div className="flex justify-center gap-2 mt-6">
          <div className="w-2 h-2 rounded-full bg-violet-500 animate-bounce" style={{ animationDelay: "0ms" }} />
          <div className="w-2 h-2 rounded-full bg-violet-500 animate-bounce" style={{ animationDelay: "150ms" }} />
          <div className="w-2 h-2 rounded-full bg-violet-500 animate-bounce" style={{ animationDelay: "300ms" }} />
        </div>
      </div>
    </div>
  );
}
