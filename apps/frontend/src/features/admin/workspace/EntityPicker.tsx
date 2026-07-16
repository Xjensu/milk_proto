import { useEffect, useId, useState } from "react";
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
  const generatedId = useId();
  const listId = `${generatedId}-options`;
  const [isOpen, setIsOpen] = useState(false);
  const selectedOption = options.find((option) => option.id === selectedId);

  useEffect(() => {
    if (disabled) setIsOpen(false);
  }, [disabled]);

  const showOptions = isOpen && options.length > 0;
  return <div className="entity-picker">
    <label htmlFor={`${listId}-input`}>{label}</label>
    <div className="entity-picker-input"><Search aria-hidden="true" size={17} /><input
      id={`${listId}-input`}
      role="combobox"
      aria-controls={listId}
      aria-expanded={showOptions}
      aria-autocomplete="list"
      value={query}
      onFocus={() => setIsOpen(true)}
      onChange={(event) => { setIsOpen(true); onQueryChange(event.target.value); }}
      onKeyDown={(event) => { if (event.key === "Escape") setIsOpen(false); }}
      onBlur={() => window.setTimeout(() => setIsOpen(false), 100)}
      disabled={disabled}
      placeholder={placeholder ?? `Поиск: ${label.toLowerCase()}`}
    /></div>
    {showOptions ? <ul id={listId} role="listbox">
      {options.map((option) => <li key={option.id} role="option" aria-selected={option.id === selectedId}>
        <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => { onSelect(option); setIsOpen(false); }}>
          <strong>{option.label}</strong>{option.description ? <span>{option.description}</span> : null}
        </button>
      </li>)}
    </ul> : null}
    {selectedOption && !isOpen ? <small className="entity-picker-selection">Выбрано: {selectedOption.label}</small> : null}
  </div>;
}
