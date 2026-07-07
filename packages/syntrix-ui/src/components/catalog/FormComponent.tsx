import { useState } from "react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import type { FormComponent } from "./types";

interface Props {
  component: FormComponent;
  initialData?: Record<string, unknown>;
  onSubmit: (data: Record<string, unknown>) => Promise<void>;
}

export function FormRenderer({ component, initialData, onSubmit }: Props) {
  const [values, setValues] = useState<Record<string, unknown>>(() => {
    const base: Record<string, unknown> = {};
    for (const field of component.fields) {
      base[field.key] = initialData?.[field.key] ?? field.defaultValue ?? "";
    }
    return base;
  });
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};
    for (const v of component.validations) {
      const val = values[v.field];
      if (v.op === "required" && (!val || val === "")) {
        newErrors[v.field] = v.message;
      }
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async () => {
    if (!validate()) return;
    setSubmitting(true);
    try {
      await onSubmit(values);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-4">
      {component.fields.map(field => (
        <div key={field.key}>
          <label className="text-xs text-muted-foreground block mb-1">
            {field.label}
            {field.required && <span className="text-destructive ml-0.5">*</span>}
          </label>
          {field.type === "text" || field.type === "number" ? (
            <Input
              type={field.type}
              value={String(values[field.key] ?? "")}
              onChange={e => setValues(prev => ({ ...prev, [field.key]: e.target.value }))}
              className={errors[field.key] ? "border-destructive" : ""}
            />
          ) : (
            <Input
              value={String(values[field.key] ?? "")}
              onChange={e => setValues(prev => ({ ...prev, [field.key]: e.target.value }))}
              className={errors[field.key] ? "border-destructive" : ""}
            />
          )}
          {errors[field.key] && (
            <p className="text-xs text-destructive mt-0.5">{errors[field.key]}</p>
          )}
        </div>
      ))}
      <div className="flex justify-end pt-2">
        <Button size="sm" onClick={handleSubmit} disabled={submitting}>
          {submitting ? "Guardando..." : component.submitLabel}
        </Button>
      </div>
    </div>
  );
}
