using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

public partial class RoutingProfilesWindow : Window
{
    private readonly ObservableCollection<RoutingProfile> _profiles;

    public RoutingProfilesWindow(IEnumerable<RoutingProfile> profiles)
    {
        InitializeComponent();
        _profiles = new ObservableCollection<RoutingProfile>(profiles);
        ProfilesList.ItemsSource = _profiles;
        if (_profiles.Count > 0)
        {
            ProfilesList.SelectedIndex = 0;
        }
    }

    public IReadOnlyList<RoutingProfile> ResultProfiles => _profiles.ToArray();

    private void ProfilesList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (ProfilesList.SelectedItem is not RoutingProfile profile)
        {
            return;
        }

        NameTextBox.Text = profile.Name;
        DescriptionTextBox.Text = profile.Description;
        ModeComboBox.SelectedIndex = profile.Mode == RoutingMode.Selective ? 1 : 0;
        KillSwitchCheckBox.IsChecked = profile.KillSwitchEnabled;
        ExclusionsTextBox.Text = string.Join(Environment.NewLine, profile.Exclusions);
        PortsTextBox.Text = string.Join(", ", profile.KillSwitchAllowPorts);
    }

    private void AddButton_Click(object sender, RoutedEventArgs e)
    {
        var profile = new RoutingProfile
        {
            Id = Guid.NewGuid().ToString("n"),
            Name = "Custom profile",
            Description = "Новый профиль"
        };
        _profiles.Add(profile);
        ProfilesList.SelectedItem = profile;
    }

    private void LocalBypassButton_Click(object sender, RoutedEventArgs e)
    {
        var profile = new RoutingProfile
        {
            Id = Guid.NewGuid().ToString("n"),
            Name = "Local bypass",
            Mode = RoutingMode.General,
            KillSwitchEnabled = true,
            Exclusions = new[] { "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16" },
            Description = "VPN, кроме локальных сетей"
        };
        _profiles.Add(profile);
        ProfilesList.SelectedItem = profile;
    }

    private void DeleteButton_Click(object sender, RoutedEventArgs e)
    {
        if (ProfilesList.SelectedItem is not RoutingProfile profile)
        {
            return;
        }

        _profiles.Remove(profile);
        ProfilesList.SelectedIndex = _profiles.Count > 0 ? 0 : -1;
    }

    private void SaveProfileButton_Click(object sender, RoutedEventArgs e)
    {
        if (ProfilesList.SelectedItem is not RoutingProfile profile)
        {
            return;
        }

        var updated = profile with
        {
            Name = UiParsing.EmptyTo(NameTextBox.Text.Trim(), "Routing profile"),
            Description = DescriptionTextBox.Text.Trim(),
            Mode = ModeComboBox.SelectedIndex == 1 ? RoutingMode.Selective : RoutingMode.General,
            KillSwitchEnabled = KillSwitchCheckBox.IsChecked == true,
            Exclusions = UiParsing.TextList(ExclusionsTextBox.Text),
            KillSwitchAllowPorts = UiParsing.Ports(PortsTextBox.Text)
        };
        var index = _profiles.IndexOf(profile);
        _profiles[index] = updated;
        ProfilesList.SelectedItem = updated;
    }

    private void DoneButton_Click(object sender, RoutedEventArgs e)
    {
        DialogResult = true;
    }
}
