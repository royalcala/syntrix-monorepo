import { useNavigate } from "react-router-dom";
import { flexRender, getCoreRowModel, getSortedRowModel, useReactTable, type SortingState } from "@tanstack/react-table";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "../ui/table";
import type { DataTableComponent } from "./types";
import { useState, useMemo } from "react";

interface Props {
  component: DataTableComponent;
  data: Record<string, unknown>[];
}

export function DataTableRenderer({ component, data }: Props) {
  const navigate = useNavigate();
  const [sorting, setSorting] = useState<SortingState>([]);

  const columns = useMemo(() => 
    component.columns
      .filter(col => col.key !== "org_id" && col.key !== "doc_id" && col.key !== "change_time" && col.key !== "node_id")
      .map(col => ({
        accessorKey: col.key,
        header: col.label,
        enableSorting: col.sortable ?? true,
        cell: ({ getValue }: any) => {
          const val = getValue();
          if (val === null || val === undefined) return <span className="text-muted-foreground">—</span>;
          return String(val);
        },
      })),
    [component.columns]
  );

  const table = useReactTable({
    data,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div className="rounded-md border border-border overflow-auto">
      <Table>
        <TableHeader>
          {table.getHeaderGroups().map(headerGroup => (
            <TableRow key={headerGroup.id}>
              {headerGroup.headers.map(header => (
                <TableHead key={header.id} onClick={header.column.getToggleSortingHandler()} className="cursor-pointer select-none">
                  {flexRender(header.column.columnDef.header, header.getContext())}
                  {{ asc: " ↑", desc: " ↓" }[header.column.getIsSorted() as string] ?? null}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {table.getRowModel().rows.length === 0 ? (
            <TableRow>
              <TableCell colSpan={columns.length} className="text-center text-muted-foreground py-8">
                Sin datos
              </TableCell>
            </TableRow>
          ) : (
            table.getRowModel().rows.map(row => (
              <TableRow
                key={row.id}
                className={component.onRowClick ? "cursor-pointer hover:bg-muted/50" : ""}
                onClick={() => {
                  if (component.onRowClick) {
                    const entity = component.onRowClick.entity;
                    const id = row.getValue("doc_id") || row.getValue("id");
                    if (id) navigate(`/${entity}?id=${id}`);
                  }
                }}
              >
                {row.getVisibleCells().map(cell => (
                  <TableCell key={cell.id}>
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}
