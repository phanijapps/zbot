import { NAME_PRESETS, type NamePreset } from "../presets";

interface NameStepProps {
  agentName: string;
  namePreset: string | null;
  onChange: (name: string, presetId: string | null) => void;
}

export function NameStep({ agentName, namePreset, onChange }: NameStepProps) {
  const handlePresetClick = (preset: NamePreset) => {
    if (preset.id === "custom") {
      onChange("", "custom");
    } else {
      onChange(preset.name, preset.id);
    }
  };

  return (
    <div>
      <div className="name-preset-grid">
        {NAME_PRESETS.map((preset) => (
          <div
            key={preset.id}
            className={`name-preset ${namePreset === preset.id ? "name-preset--selected" : ""}`}
            onClick={() => handlePresetClick(preset)}
          >
            <span className="name-preset__emoji">{preset.emoji}</span>
            <span className="name-preset__name">{preset.name}</span>
            <span className="name-preset__tagline">{preset.tagline}</span>
          </div>
        ))}
      </div>

      <div className="form-group">
        <label className="form-label">Agent Name</label>
        <input
          className="form-input"
          value={agentName}
          onChange={(e) => {
            const val = e.target.value.slice(0, 50);
            const matchingPreset = NAME_PRESETS.find((p) => p.name === val && p.id !== "custom");
            onChange(val, matchingPreset?.id || "custom");
          }}
          placeholder="Enter a name..."
          maxLength={50}
        />
        <p className="settings-hint">
          Click a preset above or type your own name. You can always change this later.
        </p>
      </div>
    </div>
  );
}
