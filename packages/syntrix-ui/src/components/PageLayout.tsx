import React from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Button } from "./ui/button";

export interface PageHeaderProps {
  /** Título principal de la página */
  title: React.ReactNode;
  /** Breve descripción o subtítulo que se muestra debajo del título */
  description?: React.ReactNode;
  /** Ruta opcional para redirigir en lugar de window.history.back() */
  backTo?: string;
  /** Controla si se debe forzar o desactivar la visualización del botón de retorno */
  showBackButton?: boolean;
  /** Acciones secundarias o principales a la derecha (botones, selectores, etc.) */
  actions?: React.ReactNode;
}

export function PageHeader({
  title,
  description,
  backTo,
  showBackButton,
  actions,
}: PageHeaderProps) {
  const navigate = useNavigate();

  // Detecta si hay historial en la SPA para poder retroceder
  const hasHistory = window.history.state && window.history.state.idx > 0;
  const canGoBack = showBackButton !== false && (backTo || hasHistory);

  const handleBack = () => {
    if (backTo) {
      navigate(backTo);
    } else {
      navigate(-1);
    }
  };

  return (
    <div className="flex flex-col gap-1 border-b border-border pb-5 mb-6">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          {canGoBack && (
            <Button
              variant="ghost"
              size="icon"
              className="-ml-2 h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground transition-colors"
              onClick={handleBack}
              aria-label="Volver"
            >
              <ArrowLeft className="h-4.5 w-4.5" />
            </Button>
          )}
          <div className="text-2xl font-bold tracking-tight text-foreground">
            {title}
          </div>
        </div>
        {actions && (
          <div className="flex items-center gap-2 animate-in fade-in duration-200">
            {actions}
          </div>
        )}
      </div>
      {description && (
        <div className="text-sm text-muted-foreground max-w-2xl">
          {description}
        </div>
      )}
    </div>
  );
}

export interface PageLayoutProps extends PageHeaderProps {
  /** Contenido principal de la página */
  children: React.ReactNode;
  /** Clases de Tailwind adicionales para el contenedor principal */
  className?: string;
  /** Clases de Tailwind adicionales para el contenedor del contenido */
  contentClassName?: string;
}

export function PageLayout({
  title,
  description,
  backTo,
  showBackButton,
  actions,
  children,
  className = "",
  contentClassName = "",
}: PageLayoutProps) {
  return (
    <div className={`p-6 max-w-7xl mx-auto w-full flex flex-col ${className}`}>
      <PageHeader
        title={title}
        description={description}
        backTo={backTo}
        showBackButton={showBackButton}
        actions={actions}
      />
      <div className={`animate-in fade-in slide-in-from-bottom-2 duration-300 ease-out ${contentClassName}`}>
        {children}
      </div>
    </div>
  );
}
