import { Card, CardContent } from "@syntrix/ui/components/ui/card";
import { Button } from "@syntrix/ui/components/ui/button";
import { Badge } from "@syntrix/ui/components/ui/badge";
import { Mail, Check } from "lucide-react";
import { PageLayout } from "@syntrix/ui/components/PageLayout";

interface InvitePayload {
  org_name: string;
  role: string;
  tickets: { ns: string; ticket: string }[];
}

export function Inbox({
  invites,
  onAccept,
}: {
  invites: InvitePayload[];
  onAccept: (invite: InvitePayload) => void;
}) {
  return (
    <PageLayout
      title={
        <div className="flex items-center gap-3">
          <Mail size={24} className="text-primary" />
          <span>Bandeja de Entrada</span>
          {invites.length > 0 && <Badge variant="destructive">{invites.length} new</Badge>}
        </div>
      }
      description="Administra y acepta invitaciones de organizaciones enviadas por administradores."
    >
      {invites.length === 0 ? (
        <Card>
          <CardContent className="py-12 text-center">
            <Mail size={40} className="text-muted-foreground mx-auto mb-4" />
            <p className="text-muted-foreground">No notifications yet.</p>
            <p className="text-xs text-muted-foreground mt-1">When an admin invites you to an org, it will appear here.</p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-3">
          {invites.map((invite, i) => (
            <Card key={i}>
              <CardContent className="flex items-center justify-between py-5">
                <div>
                  <p className="font-semibold">Organization invitation</p>
                  <p className="text-sm text-muted-foreground mt-1">
                    You have been invited to join <strong>{invite.org_name}</strong> as <Badge variant="secondary">{invite.role}</Badge>
                  </p>
                </div>
                <Button size="sm" onClick={() => onAccept(invite)}>
                  <Check size={16} /> Accept
                </Button>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </PageLayout>
  );
}
