import { Search } from "lucide-react";

export type EntityOption = { id: string; label: string; description?: string };

export function EntityPicker({ label, query, onQueryChange, options, selectedId, onSelect, disabled = false, placeholder }: {
  label: string;
  query: string;
  onQueryChange: (query: string) => void;
  options: EntityOption[];
  selectedId: string | null;
  onSelect: (option: EntityOption) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  const listId = `${label.toLowerCase().replace(/\W+/g, "-")}-options`;
  return <div className="entity-picker">
    <label htmlFor={`${listId}-input`}>{label}</label>
    <div className="entity-picker-input"><Search aria-hidden="true" size={17} /><input
      id={`${listId}-input`}
      role="combobox"
      aria-controls={listId}
      aria-expanded={options.length > 0}
      aria-autocomplete="list"
      value={query}
      onChange={(event) => onQueryChange(event.target.value)}
      disabled={disabled}
      placeholder={placeholder ?? `Search ${label.toLowerCase()}`}
    /></div>
    {options.length > 0 ? <ul id={listId} role="listbox">
      {options.map((option) => <li key={option.id} role="option" aria-selected={option.id === selectedId}>
        <button type="button" onClick={() => onSelect(option)}>
          <strong>{option.label}</strong>{option.description ? <span>{option.description}</span> : null}
        </button>
      </li>)}
    </ul> : null}
  </div>;
}
